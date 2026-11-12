// Octet's feedback relay: the app posts { kind, text, system } here and this opens a GitHub issue with a
// token the app never sees. Deploy steps: README.md next to this file.
export default {
  async fetch(req, env) {
    if (req.method !== "POST") return new Response("POST only", { status: 405 });
    const ip = req.headers.get("cf-connecting-ip") || "?";
    if (!(await env.LIMIT.limit({ key: ip })).success) return new Response("Too many reports, try again later", { status: 429 });
    let m;
    try { m = await req.json(); } catch { return new Response("Bad request", { status: 400 }); }
    const text = String(m.text || "").trim().slice(0, 4000), system = String(m.system || "").slice(0, 200);
    const idea = m.kind === "idea";
    if (text.length < 5) return new Response("Write a little more", { status: 400 });
    const title = (idea ? "Idea: " : "Bug: ") + text.split("\n")[0].slice(0, 70);
    const r = await fetch(`https://api.github.com/repos/${env.REPO}/issues`, {
      method: "POST",
      headers: { authorization: `Bearer ${env.GITHUB_TOKEN}`, accept: "application/vnd.github+json", "user-agent": "octet-feedback" },
      body: JSON.stringify({ title, body: `${text}\n\n---\nSent from Octet · ${system}`, labels: [idea ? "enhancement" : "bug"] }),
    });
    if (!r.ok) return new Response("GitHub refused the report", { status: 502 });
    return Response.json({ number: (await r.json()).number });
  },
};
