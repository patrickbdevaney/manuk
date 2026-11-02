// Thin MCP stdio client for manuk-mcp (Node built-ins only).
//   const m = new Manuk(["manuk-mcp", "--backend", "chrome"]);
//   console.log((await m.tools()).map((t) => t.name));
//   console.log(await m.call("navigate", { url: "https://example.com/" }));
//   m.close();
import { spawn, ChildProcessWithoutNullStreams } from "node:child_process";
import { createInterface } from "node:readline";

type Pending = { resolve: (v: any) => void; reject: (e: any) => void };

export class Manuk {
  private p: ChildProcessWithoutNullStreams;
  private id = 0;
  private pending = new Map<number, Pending>();
  ready: Promise<void>;

  constructor(argv: string[] = ["manuk-mcp"]) {
    this.p = spawn(argv[0], argv.slice(1), { stdio: ["pipe", "pipe", "inherit"] });
    createInterface({ input: this.p.stdout }).on("line", (line) => {
      const msg = JSON.parse(line);
      const w = this.pending.get(msg.id);
      if (!w) return; // notifications such as tools/list_changed
      this.pending.delete(msg.id);
      msg.error ? w.reject(msg.error) : w.resolve(msg.result);
    });
    this.ready = this.rpc("initialize", {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: { name: "manuk-client-ts", version: "0" },
    }).then(() => this.send({ jsonrpc: "2.0", method: "notifications/initialized" }));
  }

  private send(msg: object) {
    this.p.stdin.write(JSON.stringify(msg) + "\n");
  }

  private rpc(method: string, params: object): Promise<any> {
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.send({ jsonrpc: "2.0", id, method, params });
    });
  }

  async tools(): Promise<{ name: string }[]> {
    await this.ready;
    return (await this.rpc("tools/list", {})).tools;
  }

  async call(name: string, args: object = {}): Promise<string> {
    await this.ready;
    const r = await this.rpc("tools/call", { name, arguments: args });
    return (r.content ?? []).map((c: any) => c.text ?? "").join("");
  }

  close() {
    this.p.stdin.end();
  }
}
