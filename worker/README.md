# Feedback relay

Octet's "Send" in Settings → Feedback posts here; this Worker opens the GitHub issue. The token stays in Cloudflare.

1. GitHub → Settings → Developer settings → Fine-grained tokens → new token:
   repository access *Only octet-app/octet*, permission *Issues: Read and write*. Nothing else.
2. In this folder:
   ```
   npx wrangler login
   npx wrangler secret put GITHUB_TOKEN     # paste the token
   npx wrangler deploy
   ```
3. Wrangler prints the address (`https://octet-feedback.<account>.workers.dev`). Put its host in `FEEDBACK_HOST`
   in `crates/octet/src/main.rs`.

