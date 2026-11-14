//! RG.S1.a.a.a.b: wall ms of each zilloft /homes panel click, with the LAYOUT PHASES the click's relayouts
//! logged (`MANUK_LAYOUT_PROFILE=1`). Layout total is summed per click; the remainder of the click's wall
//! is script + cascade + a11y. Usage: `s1aaab_click_phases` (env MANUK_MAX_DRAIN_MS as for the MCP server).
use std::sync::{Arc, Mutex};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Default)]
struct Sum(Arc<Mutex<(u64, f64, f64, f64, f64)>>); // n, total, measure, taffy, unattributed

struct V<'a>(&'a mut (f64, f64, f64, f64));
impl tracing::field::Visit for V<'_> {
    fn record_f64(&mut self, f: &tracing::field::Field, v: f64) {
        match f.name() {
            "total_ms" => self.0 .0 = v,
            "measure_ms" => self.0 .1 = v,
            "taffy_ms" => self.0 .2 = v,
            "unattributed_ms" => self.0 .3 = v,
            _ => {}
        }
    }
    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}
impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Sum {
    fn on_event(&self, e: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let mut x = (0.0, 0.0, 0.0, 0.0);
        e.record(&mut V(&mut x));
        if x.0 > 0.0 {
            let mut s = self.0.lock().unwrap();
            s.0 += 1;
            s.1 += x.0;
            s.2 += x.1;
            s.3 += x.2;
            s.4 += x.3;
        }
    }
}

fn main() {
    let sum = Sum::default();
    let acc = sum.0.clone();
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(sum)).unwrap();
    std::env::set_var("MANUK_LAYOUT_PROFILE", "1");
    let rt = manuk_net::runtime();
    rt.block_on(async {
        let mut b = manuk_agent::AgentBrowser::new(1280, 800);
        b.navigate("https://evals-zilloft.vercel.app").await.unwrap();
        let tb = b
            .a11y_tree()
            .unwrap()
            .find_containing(&manuk_a11y::Role::TextBox, "Search homes")
            .unwrap()
            .node;
        b.type_into_handle(tb, "San Francisco").unwrap();
        let take = || std::mem::take(&mut *acc.lock().unwrap());
        take();
        let btn = manuk_a11y::Role::Button;
        for name in ["Start search", "Beds & Baths", "3+"] {
            let t = std::time::Instant::now();
            let r = b.click_by_name(&btn, name).await;
            let ms = t.elapsed().as_secs_f64() * 1e3;
            let (n, tot, meas, taffy, un) = take();
            println!(
                "STEP\t{name}\twall_ms={ms:.0}\tok={}\tlayouts={n}\tlayout_ms={tot:.0}\tmeasure_ms={meas:.0}\ttaffy_ms={taffy:.0}\tunattributed_ms={un:.0}\tnon_layout_ms={:.0}",
                r.is_ok(),
                ms - tot
            );
        }
    });
}
