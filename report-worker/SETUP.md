# Setting up problem reports

Encore's Help → Report a problem sends reports to a small Cloudflare Worker (`worker.js` here).
The Worker files each one as an issue in a **private** GitHub repository, so nobody can see
anyone else's report or log, and hands your replies back to the PC that sent it (Help → My
reports). This takes about 15–20 minutes, once.

## 1. The private repository

On GitHub: **New repository** → name it `Encore-reports` → choose **Private** → **Create**.
You'll get an email or GitHub notification for every new report (you watch your own
repositories by default). Reply with a comment as normal. Close the issue when it's fixed and
the reporter sees it as Closed. A comment that starts with `//` is a note for you only: Encore
never shows it.

## 2. A token that can only file issues there

GitHub → your picture → **Settings** → **Developer settings** → **Personal access tokens** →
**Fine-grained tokens** → **Generate new token**:

- Name: `Encore reports`. Expiration: 1 year (put a reminder in your calendar to renew it).
- Repository access: **Only select repositories** → `Encore-reports`.
- Permissions → Repository permissions → **Issues: Read and write**. Nothing else.
- **Generate token** and copy it (it's shown once).

## 3. The Worker

In this folder (`report-worker`), with Node installed:

```
npx wrangler login
npx wrangler deploy
npx wrangler secret put GITHUB_TOKEN     (paste the token from step 2)
npx wrangler secret put KEY_SECRET       (paste any long random text, e.g. 40 random letters)
```

Keep KEY_SECRET the same forever: changing it stops Encore reading replies to older reports.

`wrangler deploy` prints the Worker's address, like
`https://encore-reports.<your-name>.workers.dev`. Send that address over and it goes into
Encore (`REPORTS_URL` in ui/app.js). Encore's is https://encore-reports.ancillaauri.workers.dev.

## 4. Check it

```
curl -X POST https://encore-reports.<your-name>.workers.dev/report -H "content-type: application/json" -d "{\"version\":\"test\",\"what\":\"Test report, please ignore\"}"
```

It answers with `{"number":1,"key":"..."}`, and issue #1 appears in `Encore-reports`.

## Limits

- At most 3 reports a minute from one place; reports over 200 KB are refused.
- Cloudflare's free plan covers 100,000 requests a day; GitHub allows 5,000 API calls an hour
  per token. Each Encore with reports checks for replies every 30 minutes.
