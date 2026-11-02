// Encore's problem reports. Encore sends a report here; this files it as an issue in a PRIVATE
// GitHub repository (so nobody sees anyone else's log), and later hands the replies back to the
// one PC that sent it. The GitHub token lives only in this Worker's secrets, never in the app.
//
//   POST /report          { version, what, doing, details? }  ->  { number, key }
//   GET  /replies?n=&key=                                      ->  { state, replies: [{ body, at }] }
//
// `key` is a signature of the report's number, made with a secret only this Worker knows, so
// only the PC that sent a report can read its replies. Comments starting with "//" are notes for
// yourself: they're never shown in Encore.
//
// Secrets: GITHUB_TOKEN (fine-grained: Issues read and write on the reports repository only) and
// KEY_SECRET (any long random text). Variable: REPO ("owner/name" of the private repository).

const MAX_BYTES = 200_000;

export default {
  async fetch(request, env) {
    const cors = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'content-type',
    };
    const reply = (body, status = 200) =>
      new Response(JSON.stringify(body), { status, headers: { ...cors, 'content-type': 'application/json' } });
    if (request.method === 'OPTIONS') return new Response(null, { headers: cors });
    const url = new URL(request.url);
    try {
      if (url.pathname === '/report' && request.method === 'POST') return await fileReport(request, env, reply);
      if (url.pathname === '/replies' && request.method === 'GET') return await replies(url, env, reply);
      return reply({ error: 'Not found.' }, 404);
    } catch (e) {
      return reply({ error: 'Something went wrong on the report server. Try again later.' }, 500);
    }
  },
};

async function fileReport(request, env, reply) {
  // A few reports a minute from one place at most, so nobody can flood the repository.
  if (env.LIMIT) {
    const { success } = await env.LIMIT.limit({ key: request.headers.get('cf-connecting-ip') || 'unknown' });
    if (!success) return reply({ error: 'Too many reports in a short time. Try again in a minute.' }, 429);
  }
  const text = await request.text();
  if (text.length > MAX_BYTES) return reply({ error: 'That report is too big to send.' }, 413);
  let r;
  try { r = JSON.parse(text); } catch { return reply({ error: 'That report couldn\u2019t be read.' }, 400); }
  const what = clip(r.what, 4000).trim();
  if (what.length < 5) return reply({ error: 'Say what happened first.' }, 400);
  const doing = clip(r.doing, 2000).trim();
  const version = clip(r.version, 40).trim() || 'unknown';
  const d = r.details && typeof r.details === 'object' ? r.details : null;

  const firstLine = what.split('\n')[0];
  const title = `[${version}] ${firstLine.length > 70 ? `${firstLine.slice(0, 69)}\u2026` : firstLine}`;
  const lines = [
    '### What happened', quote(what), '',
    '### What they were doing', doing ? quote(doing) : '_Not said._', '',
    `**Encore** ${version}`,
  ];
  if (d) {
    lines.push(
      `**Windows** ${clip(d.windows, 80)}${d.administrator ? ' (Encore running as administrator)' : ''}`,
      `**Encoders** ${(Array.isArray(d.encoders) ? d.encoders : []).map((e) => clip(e, 30)).join(', ') || 'none found'}`,
      `**Settings** ${clip(d.settings, 600)}`,
      '',
      '<details><summary>Encore\u2019s log (the last few minutes)</summary>',
      '',
      '```text',
      scrub(clip(d.log, 150_000)).replace(/```/g, "'''"),
      '```',
      '</details>',
    );
  } else {
    lines.push('_No log or PC details were sent._');
  }
  const res = await github(env, `/repos/${env.REPO}/issues`, {
    method: 'POST',
    body: JSON.stringify({ title, body: lines.join('\n') }),
  });
  if (!res.ok) return reply({ error: 'The report couldn\u2019t be filed. Try again later.' }, 502);
  const issue = await res.json();
  return reply({ number: issue.number, key: await sign(env, issue.number) });
}

async function replies(url, env, reply) {
  const n = Number(url.searchParams.get('n'));
  const key = url.searchParams.get('key') || '';
  if (!Number.isInteger(n) || n < 1 || !same(key, await sign(env, n))) return reply({ error: 'Not found.' }, 404);
  const [issueRes, commentsRes] = await Promise.all([
    github(env, `/repos/${env.REPO}/issues/${n}`),
    github(env, `/repos/${env.REPO}/issues/${n}/comments?per_page=100`),
  ]);
  if (!issueRes.ok || !commentsRes.ok) return reply({ error: 'Replies couldn\u2019t be fetched. Try again later.' }, 502);
  const issue = await issueRes.json();
  const comments = await commentsRes.json();
  return reply({
    state: issue.state,
    replies: comments
      .filter((c) => !String(c.body || '').trimStart().startsWith('//'))
      .map((c) => ({ body: String(c.body || ''), at: c.created_at })),
  });
}

function github(env, path, init = {}) {
  return fetch(`https://api.github.com${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${env.GITHUB_TOKEN}`,
      Accept: 'application/vnd.github+json',
      'X-GitHub-Api-Version': '2022-11-28',
      'User-Agent': 'encore-reports',
      'Content-Type': 'application/json',
    },
  });
}

// The report's key: HMAC-SHA256 of its number with KEY_SECRET (first 32 hex characters).
async function sign(env, n) {
  const k = await crypto.subtle.importKey('raw', new TextEncoder().encode(env.KEY_SECRET), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  const mac = new Uint8Array(await crypto.subtle.sign('HMAC', k, new TextEncoder().encode(`report:${n}`)));
  return [...mac.slice(0, 16)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

// Compares two keys in the same time whatever they hold.
function same(a, b) {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}

const clip = (v, max) => (typeof v === 'string' ? v : '').slice(0, max);
const quote = (t) => t.split('\n').map((l) => `> ${l}`).join('\n');
// Encore already takes user names out of paths; this makes sure.
const scrub = (t) => t.replace(/(\\users\\)[^\\/"'\n)\]]+/gi, '$1<user>');

export const _test = { sign, same, scrub, quote };
