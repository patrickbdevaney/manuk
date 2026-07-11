//! Pure-Rust WOFF2 (and WOFF1) web-font decoding to sfnt (TTF/OTF) bytes.
//!
//! Real web fonts ship as WOFF2 — a Brotli-compressed sfnt with the `glyf`/`loca`
//! tables further shrunk by a domain-specific "transform" (points/flags/contours
//! packed as varints). `fontdb`/`ttf-parser`/swash only understand raw sfnt, so we
//! decompress + reconstruct here before registering the font.
//!
//! This is a hand-rolled port of the WOFF2 spec's reconstruction algorithm
//! (https://www.w3.org/TR/WOFF2/), cross-checked against Google's reference
//! `woff2_dec.cc`. It depends only on `brotli-decompressor` (decode-only, safe,
//! pure Rust) for the compressed block and `flate2` (zlib) for legacy WOFF1 —
//! no C/C++ toolchain, no heavy font library.
//!
//! Supported transforms: the `glyf`/`loca` transform (version 0) and the `hmtx`
//! transform (version 1). Null-transform tables are copied verbatim. TrueType
//! collections (`ttcf`) are not reconstructed (rare for web fonts) — `None`.

/// The 63 known table tags, indexed by the low 6 bits of a table's flag byte.
/// Order is normative (WOFF2 spec §5.2); a flag value of `0x3f` means an
/// arbitrary 4-byte tag follows instead.
#[rustfmt::skip]
const KNOWN_TAGS: [&[u8; 4]; 63] = [
    b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name", b"OS/2", b"post",
    b"cvt ", b"fpgm", b"glyf", b"loca", b"prep", b"CFF ", b"VORG", b"EBDT",
    b"EBLC", b"gasp", b"hdmx", b"kern", b"LTSH", b"PCLT", b"VDMX", b"vhea",
    b"vmtx", b"BASE", b"GDEF", b"GPOS", b"GSUB", b"EBSC", b"JSTF", b"MATH",
    b"CBDT", b"CBLC", b"COLR", b"CPAL", b"SVG ", b"sbix", b"acnt", b"avar",
    b"bdat", b"bloc", b"bsln", b"cvar", b"fdsc", b"feat", b"fmtx", b"fvar",
    b"gvar", b"hsty", b"just", b"lcar", b"mort", b"morx", b"opbd", b"prop",
    b"trak", b"Zapf", b"Silf", b"Glat", b"Gloc", b"Feat", b"Sill",
];

/// A minimal big-endian byte cursor over a slice; every read is bounds-checked
/// and returns `None` on truncation.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        let b = self.take(2)?;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }
    fn i16(&mut self) -> Option<i16> {
        Some(self.u16()? as i16)
    }
    fn u32(&mut self) -> Option<u32> {
        let b = self.take(4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }

    /// WOFF2 `UIntBase128`: a big-endian variable-length uint (1–5 bytes, 7 bits each).
    fn base128(&mut self) -> Option<u32> {
        let mut result: u32 = 0;
        for i in 0..5 {
            let code = self.u8()?;
            // Leading 0x80 (a leading zero) is invalid per spec.
            if i == 0 && code == 0x80 {
                return None;
            }
            // Overflow guard: any of the top seven bits set means the next shift overflows.
            if result & 0xfe00_0000 != 0 {
                return None;
            }
            result = (result << 7) | (code & 0x7f) as u32;
            if code & 0x80 == 0 {
                return Some(result);
            }
        }
        None
    }

    /// WOFF2 `255UInt16`: a variable-length uint16 (MicroType Express §6.1.1).
    fn u255(&mut self) -> Option<u32> {
        const WORD: u8 = 253;
        const ONE_MORE_2: u8 = 254;
        const ONE_MORE_1: u8 = 255;
        const LOWEST: u32 = 253;
        let code = self.u8()?;
        match code {
            WORD => Some(self.u16()? as u32),
            ONE_MORE_1 => Some(self.u8()? as u32 + LOWEST),
            ONE_MORE_2 => Some(self.u8()? as u32 + LOWEST * 2),
            _ => Some(code as u32),
        }
    }
}

/// A parsed WOFF2 table-directory entry (offsets are into the decompressed block).
struct TableDir {
    tag: [u8; 4],
    transformed: bool,
    transform_length: usize,
    dst_length: usize,
    src_offset: usize,
}

/// sfnt table checksum: the sum (wrapping) of the table's data read as big-endian
/// u32s, with the tail zero-padded to a 4-byte boundary.
fn checksum(data: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        sum = sum.wrapping_add(u32::from_be_bytes([c[0], c[1], c[2], c[3]]));
    }
    let rem = chunks.remainder();
    if !rem.is_empty() {
        let mut buf = [0u8; 4];
        buf[..rem.len()].copy_from_slice(rem);
        sum = sum.wrapping_add(u32::from_be_bytes(buf));
    }
    sum
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

/// Decode a WOFF2 font to reconstructed sfnt (TTF/OTF) bytes, or `None` if the
/// input is malformed / uses unsupported features (e.g. a `ttcf` collection).
pub fn decode_woff2(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != b"wOF2" {
        return None;
    }
    let flavor = r.u32()?; // sfnt version to emit (0x00010000 TrueType, 'OTTO', 'ttcf')
    let _length = r.u32()?;
    let num_tables = r.u16()? as usize;
    if num_tables == 0 {
        return None;
    }
    r.skip(2)?; // reserved
    let _total_sfnt_size = r.u32()?;
    let compressed_length = r.u32()? as usize;
    r.skip(2 * 2)?; // major/minor version
    let _meta_offset = r.u32()?;
    let _meta_length = r.u32()?;
    let _meta_orig = r.u32()?;
    let _priv_offset = r.u32()?;
    let _priv_length = r.u32()?;

    // Collections need per-font offset tables we don't rebuild; bail cleanly.
    if flavor == u32::from_be_bytes(*b"ttcf") {
        return None;
    }

    // --- Table directory: flag byte + optional tag + varint length(s). ---
    let mut dirs: Vec<TableDir> = Vec::with_capacity(num_tables);
    let mut src_offset = 0usize;
    for _ in 0..num_tables {
        let flag = r.u8()?;
        let tag: [u8; 4] = if flag & 0x3f == 0x3f {
            let t = r.take(4)?;
            [t[0], t[1], t[2], t[3]]
        } else {
            *KNOWN_TAGS[(flag & 0x3f) as usize]
        };
        let xform_version = (flag >> 6) & 0x03;
        let is_glyf_or_loca = &tag == b"glyf" || &tag == b"loca";
        // glyf/loca carry a transform iff version 0; every other table iff version != 0.
        let transformed = if is_glyf_or_loca {
            xform_version == 0
        } else {
            xform_version != 0
        };
        let dst_length = r.base128()? as usize;
        let transform_length = if transformed {
            let tl = r.base128()? as usize;
            // A transformed loca carries no data of its own (rebuilt from glyf).
            if &tag == b"loca" && tl != 0 {
                return None;
            }
            tl
        } else {
            dst_length
        };
        dirs.push(TableDir {
            tag,
            transformed,
            transform_length,
            dst_length,
            src_offset,
        });
        src_offset = src_offset.checked_add(transform_length)?;
    }
    let uncompressed_size = src_offset;

    // --- Brotli-decompress the single data block. ---
    let comp = r.take(compressed_length)?;
    let mut block: Vec<u8> = Vec::with_capacity(uncompressed_size);
    brotli_decompressor::BrotliDecompress(&mut &comp[..], &mut block).ok()?;
    if block.len() != uncompressed_size {
        return None;
    }

    // --- Reconstruct each table's final bytes (order-independent). ---
    // Slices of the decompressed block, keyed by tag for dependency lookups.
    let mut raw: std::collections::HashMap<[u8; 4], &[u8]> = std::collections::HashMap::new();
    for d in &dirs {
        let end = d.src_offset.checked_add(d.transform_length)?;
        raw.insert(d.tag, block.get(d.src_offset..end)?);
    }

    // Final per-table data, by tag.
    let mut out_tables: std::collections::HashMap<[u8; 4], Vec<u8>> =
        std::collections::HashMap::new();

    // numberOfHMetrics lives at offset 34 of hhea; needed for an hmtx transform.
    let num_hmetrics = raw
        .get(b"hhea" as &[u8; 4])
        .and_then(|h| h.get(34..36))
        .map(|b| u16::from_be_bytes([b[0], b[1]]));

    // glyf/loca transform (the hard part).
    let mut x_mins: Vec<i16> = Vec::new();
    let mut num_glyphs: u16 = 0;
    let glyf_transformed = dirs.iter().any(|d| &d.tag == b"glyf" && d.transformed);
    if glyf_transformed {
        let glyf_raw = *raw.get(b"glyf" as &[u8; 4])?;
        let recon = reconstruct_glyf(glyf_raw)?;
        num_glyphs = recon.num_glyphs;
        x_mins = recon.x_mins;
        out_tables.insert(*b"glyf", recon.glyf);
        out_tables.insert(*b"loca", recon.loca);
    }

    for d in &dirs {
        if out_tables.contains_key(&d.tag) {
            continue; // already produced (glyf/loca)
        }
        let data = *raw.get(&d.tag)?;
        let final_bytes = if d.transformed && &d.tag == b"hmtx" {
            reconstruct_hmtx(data, num_glyphs, num_hmetrics?, &x_mins)?
        } else if d.transformed && (&d.tag == b"glyf" || &d.tag == b"loca") {
            // A transformed glyf/loca but no glyf reconstruction happened (e.g. loca
            // without glyf) — malformed.
            return None;
        } else if d.transformed {
            // Unknown transform on some other table; we can't reconstruct it.
            return None;
        } else {
            let v = data.to_vec();
            if v.len() != d.dst_length {
                return None;
            }
            v
        };
        out_tables.insert(d.tag, final_bytes);
    }

    // --- Assemble the sfnt. ---
    Some(assemble_sfnt(flavor, &dirs, out_tables))
}

/// Result of reconstructing the transformed `glyf`/`loca` pair.
struct GlyfRecon {
    glyf: Vec<u8>,
    loca: Vec<u8>,
    x_mins: Vec<i16>,
    num_glyphs: u16,
}

/// A reconstructed glyph outline point.
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
    on_curve: bool,
}

// Simple-glyph flag bits.
const FLAG_ON_CURVE: u8 = 0x01;
const FLAG_X_SHORT: u8 = 0x02;
const FLAG_Y_SHORT: u8 = 0x04;
const FLAG_REPEAT: u8 = 0x08;
const FLAG_X_SAME: u8 = 0x10;
const FLAG_Y_SAME: u8 = 0x20;
const FLAG_OVERLAP: u8 = 0x40;

// Composite-glyph flag bits.
const ARG_1_AND_2_ARE_WORDS: u16 = 1 << 0;
const WE_HAVE_A_SCALE: u16 = 1 << 3;
const MORE_COMPONENTS: u16 = 1 << 5;
const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 1 << 6;
const WE_HAVE_A_TWO_BY_TWO: u16 = 1 << 7;
const WE_HAVE_INSTRUCTIONS: u16 = 1 << 8;

/// Rebuild the `glyf` + `loca` tables from the WOFF2 glyf transform (spec §5.1).
fn reconstruct_glyf(data: &[u8]) -> Option<GlyfRecon> {
    let mut hdr = Reader::new(data);
    let _version = hdr.u16()?;
    let opt_flags = hdr.u16()?;
    let has_overlap_bitmap = opt_flags & 0x0001 != 0;
    let num_glyphs = hdr.u16()?;
    let index_format = hdr.u16()?;

    // Seven length-prefixed substreams follow the 8-byte header.
    let mut sub: Vec<&[u8]> = Vec::with_capacity(7);
    let mut offset: usize = 4 * (2 + 7); // 8-byte header + 7 u32 sizes
    for _ in 0..7 {
        let size = hdr.u32()? as usize;
        let start = offset;
        let end = start.checked_add(size)?;
        sub.push(data.get(start..end)?);
        offset = end;
    }
    let mut n_contour_stream = Reader::new(sub[0]);
    let mut n_points_stream = Reader::new(sub[1]);
    let mut flag_stream = Reader::new(sub[2]);
    let mut glyph_stream = Reader::new(sub[3]);
    let mut composite_stream = Reader::new(sub[4]);
    let mut bbox_stream = Reader::new(sub[5]);
    let mut instruction_stream = Reader::new(sub[6]);

    // Optional overlap bitmap trails the substreams.
    let overlap_bitmap: Option<&[u8]> = if has_overlap_bitmap {
        let len = (num_glyphs as usize + 7) >> 3;
        let end = offset.checked_add(len)?;
        Some(data.get(offset..end)?)
    } else {
        None
    };

    // The bbox bitmap is the leading region of the bbox substream.
    let bitmap_len = ((num_glyphs as usize + 31) >> 5) << 2;
    let bbox_bitmap = bbox_stream.take(bitmap_len)?;

    let mut glyf: Vec<u8> = Vec::new();
    let mut loca_values: Vec<u32> = Vec::with_capacity(num_glyphs as usize + 1);
    let mut x_mins: Vec<i16> = vec![0; num_glyphs as usize];

    for i in 0..num_glyphs as usize {
        loca_values.push(glyf.len() as u32);
        let have_bbox = bbox_bitmap[i >> 3] & (0x80 >> (i & 7)) != 0;
        let n_contours = n_contour_stream.u16()?;

        if n_contours == 0xffff {
            // Composite glyph — must carry an explicit bbox.
            if !have_bbox {
                return None;
            }
            let (composite_size, have_instructions) = size_of_composite(composite_stream.data
                .get(composite_stream.pos..)?)?;
            let instr_size = if have_instructions {
                glyph_stream.u255()? as usize
            } else {
                0
            };
            let mut g: Vec<u8> = Vec::with_capacity(10 + composite_size + instr_size);
            g.extend_from_slice(&(-1i16).to_be_bytes()); // 0xffff
            let bbox = bbox_stream.take(8)?;
            g.extend_from_slice(bbox);
            x_mins[i] = i16::from_be_bytes([bbox[0], bbox[1]]);
            let comp = composite_stream.take(composite_size)?;
            g.extend_from_slice(comp);
            if have_instructions {
                g.extend_from_slice(&(instr_size as u16).to_be_bytes());
                g.extend_from_slice(instruction_stream.take(instr_size)?);
            }
            glyf.extend_from_slice(&g);
        } else if n_contours > 0 {
            // Simple glyph.
            let n_contours = n_contours as usize;
            let mut endpoints: Vec<u16> = Vec::with_capacity(n_contours);
            let mut total_points: usize = 0;
            for _ in 0..n_contours {
                let n = n_points_stream.u255()? as usize;
                total_points = total_points.checked_add(n)?;
                if total_points == 0 || total_points - 1 >= 65536 {
                    return None;
                }
                endpoints.push((total_points - 1) as u16);
            }

            // Per-point flags, then triplet-encoded coordinates.
            let flags = flag_stream.take(total_points)?;
            let points = triplet_decode(flags, &mut glyph_stream, total_points)?;
            let instr_size = glyph_stream.u255()? as usize;

            // bbox: explicit or computed from the points.
            let bbox: [u8; 8] = if have_bbox {
                let b = bbox_stream.take(8)?;
                [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]
            } else {
                compute_bbox(&points)
            };
            x_mins[i] = i16::from_be_bytes([bbox[0], bbox[1]]);

            let has_overlap = has_overlap_bitmap
                && overlap_bitmap
                    .map(|b| b[i >> 3] & (0x80 >> (i & 7)) != 0)
                    .unwrap_or(false);

            let instructions = instruction_stream.take(instr_size)?;

            let mut g: Vec<u8> = Vec::new();
            g.extend_from_slice(&(n_contours as u16).to_be_bytes());
            g.extend_from_slice(&bbox);
            for e in &endpoints {
                g.extend_from_slice(&e.to_be_bytes());
            }
            g.extend_from_slice(&(instr_size as u16).to_be_bytes());
            g.extend_from_slice(instructions);
            encode_points(&points, has_overlap, &mut g);
            glyf.extend_from_slice(&g);
        } else {
            // n_contours == 0: empty glyph, must not have a bbox.
            if have_bbox {
                return None;
            }
        }
        pad4(&mut glyf);
    }
    loca_values.push(glyf.len() as u32);

    // Build loca in the declared index format.
    let mut loca: Vec<u8> = Vec::new();
    if index_format == 0 {
        for v in &loca_values {
            loca.extend_from_slice(&((v >> 1) as u16).to_be_bytes());
        }
    } else {
        for v in &loca_values {
            loca.extend_from_slice(&v.to_be_bytes());
        }
    }

    Some(GlyfRecon {
        glyf,
        loca,
        x_mins,
        num_glyphs,
    })
}

/// Walk a composite glyph's component records to measure its byte length and
/// learn whether an instruction block follows (spec §5.1 / OT composite format).
fn size_of_composite(buf: &[u8]) -> Option<(usize, bool)> {
    let mut r = Reader::new(buf);
    let mut have_instructions = false;
    let mut flags = MORE_COMPONENTS;
    while flags & MORE_COMPONENTS != 0 {
        flags = r.u16()?;
        have_instructions |= flags & WE_HAVE_INSTRUCTIONS != 0;
        r.skip(2)?; // glyph index
        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            r.skip(4)?;
        } else {
            r.skip(2)?;
        }
        if flags & WE_HAVE_A_SCALE != 0 {
            r.skip(2)?;
        } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
            r.skip(4)?;
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            r.skip(8)?;
        }
    }
    Some((r.pos, have_instructions))
}

fn with_sign(flag: u8, base: i32) -> i32 {
    if flag & 1 != 0 {
        base
    } else {
        -base
    }
}

/// Decode `n_points` triplet-encoded points from the glyph substream (spec §5.2).
/// `flags` is the per-point flag stream; coordinate bytes are consumed from `stream`.
fn triplet_decode(flags: &[u8], stream: &mut Reader, n_points: usize) -> Option<Vec<Point>> {
    let mut points = Vec::with_capacity(n_points);
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    for i in 0..n_points {
        let raw = *flags.get(i)?;
        let on_curve = raw >> 7 == 0;
        let flag = raw & 0x7f;
        let (dx, dy) = if flag < 10 {
            let b = stream.u8()? as i32;
            (0, with_sign(flag, ((flag as i32 & 14) << 7) + b))
        } else if flag < 20 {
            let b = stream.u8()? as i32;
            (with_sign(flag, (((flag as i32 - 10) & 14) << 7) + b), 0)
        } else if flag < 84 {
            let b0 = flag as i32 - 20;
            let b1 = stream.u8()? as i32;
            (
                with_sign(flag, 1 + (b0 & 0x30) + (b1 >> 4)),
                with_sign(flag >> 1, 1 + ((b0 & 0x0c) << 2) + (b1 & 0x0f)),
            )
        } else if flag < 120 {
            let b0 = flag as i32 - 84;
            let c0 = stream.u8()? as i32;
            let c1 = stream.u8()? as i32;
            (
                with_sign(flag, 1 + ((b0 / 12) << 8) + c0),
                with_sign(flag >> 1, 1 + (((b0 % 12) >> 2) << 8) + c1),
            )
        } else if flag < 124 {
            let c0 = stream.u8()? as i32;
            let c1 = stream.u8()? as i32;
            let c2 = stream.u8()? as i32;
            (
                with_sign(flag, (c0 << 4) + (c1 >> 4)),
                with_sign(flag >> 1, ((c1 & 0x0f) << 8) + c2),
            )
        } else {
            let c0 = stream.u8()? as i32;
            let c1 = stream.u8()? as i32;
            let c2 = stream.u8()? as i32;
            let c3 = stream.u8()? as i32;
            (
                with_sign(flag, (c0 << 8) + c1),
                with_sign(flag >> 1, (c2 << 8) + c3),
            )
        };
        x = x.checked_add(dx)?;
        y = y.checked_add(dy)?;
        points.push(Point { x, y, on_curve });
    }
    Some(points)
}

/// Bounding box of the points, as an 8-byte big-endian `[xMin,yMin,xMax,yMax]`.
fn compute_bbox(points: &[Point]) -> [u8; 8] {
    let (mut xmin, mut ymin, mut xmax, mut ymax) = if let Some(p) = points.first() {
        (p.x, p.y, p.x, p.y)
    } else {
        (0, 0, 0, 0)
    };
    for p in points.iter().skip(1) {
        xmin = xmin.min(p.x);
        ymin = ymin.min(p.y);
        xmax = xmax.max(p.x);
        ymax = ymax.max(p.y);
    }
    let mut out = [0u8; 8];
    out[0..2].copy_from_slice(&(xmin as i16).to_be_bytes());
    out[2..4].copy_from_slice(&(ymin as i16).to_be_bytes());
    out[4..6].copy_from_slice(&(xmax as i16).to_be_bytes());
    out[6..8].copy_from_slice(&(ymax as i16).to_be_bytes());
    out
}

/// Re-encode outline points into the sfnt simple-glyph flag+coordinate arrays
/// (with the standard REPEAT compression), appending to `out`.
fn encode_points(points: &[Point], has_overlap: bool, out: &mut Vec<u8>) {
    let mut flags: Vec<u8> = Vec::new();
    let mut xs: Vec<u8> = Vec::new();
    let mut ys: Vec<u8> = Vec::new();
    let mut last_x = 0i32;
    let mut last_y = 0i32;
    let mut last_flag: i32 = -1;
    let mut repeat_count: u32 = 0;

    for (i, p) in points.iter().enumerate() {
        let mut flag: u8 = if p.on_curve { FLAG_ON_CURVE } else { 0 };
        if has_overlap && i == 0 {
            flag |= FLAG_OVERLAP;
        }
        let dx = p.x - last_x;
        let dy = p.y - last_y;
        if dx == 0 {
            flag |= FLAG_X_SAME;
        } else if dx > -256 && dx < 256 {
            flag |= FLAG_X_SHORT;
            if dx > 0 {
                flag |= FLAG_X_SAME;
            }
            xs.push(dx.unsigned_abs() as u8);
        } else {
            xs.extend_from_slice(&(dx as i16).to_be_bytes());
        }
        if dy == 0 {
            flag |= FLAG_Y_SAME;
        } else if dy > -256 && dy < 256 {
            flag |= FLAG_Y_SHORT;
            if dy > 0 {
                flag |= FLAG_Y_SAME;
            }
            ys.push(dy.unsigned_abs() as u8);
        } else {
            ys.extend_from_slice(&(dy as i16).to_be_bytes());
        }

        if flag as i32 == last_flag && repeat_count != 255 {
            // Extend the current run: mark REPEAT on the run's flag byte (idempotent).
            if let Some(last) = flags.last_mut() {
                *last |= FLAG_REPEAT;
            }
            repeat_count += 1;
        } else {
            if repeat_count != 0 {
                flags.push(repeat_count as u8);
                repeat_count = 0;
            }
            flags.push(flag);
        }
        last_x = p.x;
        last_y = p.y;
        last_flag = flag as i32;
    }
    if repeat_count != 0 {
        flags.push(repeat_count as u8);
    }

    out.extend_from_slice(&flags);
    out.extend_from_slice(&xs);
    out.extend_from_slice(&ys);
}

/// Rebuild `hmtx` from the WOFF2 hmtx transform (spec §5.3): advance widths are
/// always present; left-side bearings may be omitted and taken from glyph `xMin`s.
fn reconstruct_hmtx(
    data: &[u8],
    num_glyphs: u16,
    num_hmetrics: u16,
    x_mins: &[i16],
) -> Option<Vec<u8>> {
    let mut r = Reader::new(data);
    let flags = r.u8()?;
    if flags & 0xfc != 0 {
        return None; // reserved bits must be zero
    }
    let has_proportional_lsbs = flags & 1 == 0;
    let has_monospace_lsbs = flags & 2 == 0;
    if has_proportional_lsbs && has_monospace_lsbs {
        return None; // claims transform but omits nothing
    }
    if num_hmetrics < 1 || num_hmetrics > num_glyphs {
        return None;
    }
    if x_mins.len() < num_glyphs as usize {
        return None;
    }

    let num_glyphs = num_glyphs as usize;
    let num_hmetrics = num_hmetrics as usize;

    let mut advances: Vec<u16> = Vec::with_capacity(num_hmetrics);
    for _ in 0..num_hmetrics {
        advances.push(r.u16()?);
    }
    let mut lsbs: Vec<i16> = Vec::with_capacity(num_glyphs);
    for i in 0..num_hmetrics {
        lsbs.push(if has_proportional_lsbs {
            r.i16()?
        } else {
            x_mins[i]
        });
    }
    for i in num_hmetrics..num_glyphs {
        lsbs.push(if has_monospace_lsbs {
            r.i16()?
        } else {
            x_mins[i]
        });
    }

    let mut out: Vec<u8> = Vec::with_capacity(2 * num_glyphs + 2 * num_hmetrics);
    for i in 0..num_glyphs {
        if i < num_hmetrics {
            out.extend_from_slice(&advances[i].to_be_bytes());
        }
        out.extend_from_slice(&lsbs[i].to_be_bytes());
    }
    Some(out)
}

/// Assemble a valid sfnt: offset table + tag-sorted table records (with correct
/// checksums and offsets) + 4-byte-padded table data, then patch the `head`
/// table's `checkSumAdjustment`.
fn assemble_sfnt(
    flavor: u32,
    dirs: &[TableDir],
    mut tables: std::collections::HashMap<[u8; 4], Vec<u8>>,
) -> Vec<u8> {
    // Preserve one record per directory tag, sorted by tag (OT spec order).
    let mut tags: Vec<[u8; 4]> = dirs.iter().map(|d| d.tag).collect();
    tags.sort_unstable();
    tags.dedup();
    let num_tables = tags.len();

    // Zero head.checkSumAdjustment before summing (patched at the end).
    if let Some(head) = tables.get_mut(b"head" as &[u8; 4]) {
        if head.len() >= 12 {
            head[8..12].copy_from_slice(&[0, 0, 0, 0]);
        }
    }

    // Offset-table search params.
    let mut max_pow2 = 0u32;
    while (1u32 << (max_pow2 + 1)) <= num_tables as u32 {
        max_pow2 += 1;
    }
    let search_range = ((1u32 << max_pow2) << 4) as u16;
    let entry_selector = max_pow2 as u16;
    let range_shift = ((num_tables as u32) << 4).wrapping_sub(search_range as u32) as u16;

    let header_size = 12 + 16 * num_tables;
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&flavor.to_be_bytes());
    out.extend_from_slice(&(num_tables as u16).to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());

    // Lay out data offsets (4-byte aligned) and emit records.
    let mut running = header_size;
    let mut head_offset: Option<usize> = None;
    let mut records: Vec<(usize, usize)> = Vec::with_capacity(num_tables); // (data_offset, len)
    for tag in &tags {
        let data = tables.get(tag).map(|v| v.as_slice()).unwrap_or(&[]);
        let len = data.len();
        let csum = checksum(data);
        out.extend_from_slice(tag);
        out.extend_from_slice(&csum.to_be_bytes());
        out.extend_from_slice(&(running as u32).to_be_bytes());
        out.extend_from_slice(&(len as u32).to_be_bytes());
        if tag == b"head" {
            head_offset = Some(running);
        }
        records.push((running, len));
        running += len;
        while running % 4 != 0 {
            running += 1;
        }
    }

    // Emit the padded table data.
    for (tag, (_off, _len)) in tags.iter().zip(records.iter()) {
        let data = tables.get(tag).map(|v| v.as_slice()).unwrap_or(&[]);
        out.extend_from_slice(data);
        pad4(&mut out);
    }

    // head.checkSumAdjustment = 0xB1B0AFBA - checksum(whole file).
    if let Some(ho) = head_offset {
        let file_sum = checksum(&out);
        let adj = 0xB1B0_AFBAu32.wrapping_sub(file_sum);
        if ho + 12 <= out.len() {
            out[ho + 8..ho + 12].copy_from_slice(&adj.to_be_bytes());
        }
    }

    out
}

/// Decode a WOFF1 font (zlib-per-table) to sfnt bytes, or `None` if malformed.
pub fn decode_woff1(bytes: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read as _;

    let mut r = Reader::new(bytes);
    if r.take(4)? != b"wOFF" {
        return None;
    }
    let flavor = r.u32()?;
    let _length = r.u32()?;
    let num_tables = r.u16()? as usize;
    r.skip(2)?; // reserved
    let _total_sfnt_size = r.u32()?;
    r.skip(2 * 2)?; // major/minor
    let _meta_offset = r.u32()?;
    let _meta_length = r.u32()?;
    let _meta_orig = r.u32()?;
    let _priv_offset = r.u32()?;
    let _priv_length = r.u32()?;

    // WOFF1 table directory: tag, offset, compLength, origLength, origChecksum.
    struct W1 {
        tag: [u8; 4],
        offset: usize,
        comp_len: usize,
        orig_len: usize,
    }
    let mut entries: Vec<W1> = Vec::with_capacity(num_tables);
    for _ in 0..num_tables {
        let t = r.take(4)?;
        let tag = [t[0], t[1], t[2], t[3]];
        let offset = r.u32()? as usize;
        let comp_len = r.u32()? as usize;
        let orig_len = r.u32()? as usize;
        let _orig_checksum = r.u32()?;
        entries.push(W1 {
            tag,
            offset,
            comp_len,
            orig_len,
        });
    }

    let mut tables: std::collections::HashMap<[u8; 4], Vec<u8>> = std::collections::HashMap::new();
    let mut dirs: Vec<TableDir> = Vec::with_capacity(num_tables);
    for e in &entries {
        let end = e.offset.checked_add(e.comp_len)?;
        let raw = bytes.get(e.offset..end)?;
        let data = if e.comp_len == e.orig_len {
            raw.to_vec() // stored uncompressed
        } else {
            let mut dec = flate2::read::ZlibDecoder::new(raw);
            let mut buf = Vec::with_capacity(e.orig_len);
            dec.read_to_end(&mut buf).ok()?;
            buf
        };
        if data.len() != e.orig_len {
            return None;
        }
        dirs.push(TableDir {
            tag: e.tag,
            transformed: false,
            transform_length: e.orig_len,
            dst_length: e.orig_len,
            src_offset: 0,
        });
        tables.insert(e.tag, data);
    }

    Some(assemble_sfnt(flavor, &dirs, tables))
}

/// Decode any supported web-font container (WOFF2 or WOFF1) to sfnt bytes.
/// Returns `None` for unrecognized input (the caller keeps raw sfnt as-is).
pub fn decode_webfont(bytes: &[u8]) -> Option<Vec<u8>> {
    match bytes.get(..4)? {
        b"wOF2" => decode_woff2(bytes),
        b"wOFF" => decode_woff1(bytes),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base128_and_255ushort() {
        // 255UInt16 word-code path.
        let mut r = Reader::new(&[253, 0x01, 0x02]);
        assert_eq!(r.u255(), Some(0x0102));
        // base128: 0x3f = 63.
        let mut r = Reader::new(&[0x3f]);
        assert_eq!(r.base128(), Some(63));
        // base128: 0x81 0x00 = (1<<7) = 128.
        let mut r = Reader::new(&[0x81, 0x00]);
        assert_eq!(r.base128(), Some(128));
    }

    #[test]
    fn decode_ahem_woff2_to_valid_sfnt() {
        let woff2 = include_bytes!("../tests/fixtures/Ahem.woff2");
        assert_eq!(&woff2[..4], b"wOF2");
        let sfnt = decode_woff2(woff2).expect("Ahem.woff2 should decode");
        // Reconstructed sfnt must start with a TrueType magic.
        assert_eq!(&sfnt[..4], &[0x00, 0x01, 0x00, 0x00]);
        // ttf-parser (via fontdb) must parse it and expose a family name.
        let mut db = fontdb::Database::new();
        db.load_font_data(sfnt);
        // fontdb only yields a face if ttf-parser parsed the sfnt and read its
        // name table — so a non-empty family name proves valid, loadable sfnt.
        let face = db.faces().next().expect("one face registered");
        assert!(
            face.families.iter().any(|(name, _)| !name.is_empty()),
            "decoded face must have a family name"
        );
        // And the face data round-trips through swash (the actual rasterizer path)
        // and yields a real outline via ttf-parser (exercises the glyf/loca transform).
        let id = face.id;
        let ok = db
            .with_face_data(id, |data, index| {
                let swash_ok = swash::FontRef::from_index(data, index as usize).is_some();
                let ttf = ttf_parser::Face::parse(data, index).expect("ttf-parser parses sfnt");
                // Some glyph in the reconstructed glyf table must produce outline commands.
                struct Sink(u32);
                impl ttf_parser::OutlineBuilder for Sink {
                    fn move_to(&mut self, _: f32, _: f32) { self.0 += 1; }
                    fn line_to(&mut self, _: f32, _: f32) { self.0 += 1; }
                    fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) { self.0 += 1; }
                    fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) { self.0 += 1; }
                    fn close(&mut self) {}
                }
                let mut outlined = false;
                for g in 0..ttf.number_of_glyphs() {
                    let mut s = Sink(0);
                    if ttf.outline_glyph(ttf_parser::GlyphId(g), &mut s).is_some() && s.0 > 0 {
                        outlined = true;
                        break;
                    }
                }
                swash_ok && outlined
            })
            .unwrap_or(false);
        assert!(ok, "reconstructed face must load in swash and have outlined glyphs");
    }
}
