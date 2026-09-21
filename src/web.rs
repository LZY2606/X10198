const HTML_PART1: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>相位织图</title>
<style>
  :root { --ink:#1d2b3a; --muted:#66788c; --line:#d8e0e8; --accent:#2b6cb0;
          --warn:#b7791f; --err:#c53030; --ok:#2f855a; --bg:#f5f8fb; }
  * { box-sizing: border-box; }
  body { margin:0; font-family: -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif;
         color:var(--ink); background:var(--bg); }
  header { padding:14px 22px; background:#12304d; color:#fff; display:flex;
           align-items:baseline; gap:14px; }
  header h1 { margin:0; font-size:20px; letter-spacing:2px; }
  header .meta { font-size:12px; opacity:.85; }
  main { padding:18px 22px; display:grid; grid-template-columns: 320px 1fr;
         gap:16px; align-items:start; }
  .card { background:#fff; border:1px solid var(--line); border-radius:10px;
          padding:14px 16px; margin-bottom:16px; }
  .card h2 { margin:0 0 10px; font-size:15px; }
  table { border-collapse:collapse; width:100%; font-size:12px; }
  th, td { border:1px solid var(--line); padding:3px 7px; text-align:center; }
  th { background:#eef3f8; position:sticky; top:0; }
  td.miss { color:var(--muted); background:#fafafa; }
  td.lowq { background:#fff7e6; }
  td.ignored { text-decoration:line-through; color:var(--err); }
  .conflict { border-left:4px solid var(--warn); padding:8px 10px; margin:7px 0;
             background:#fffaf0; border-radius:0 6px 6px 0; font-size:12.5px; }
  .conflict.denovo, .conflict.mendelian { border-color:var(--err); background:#fff5f5; }
  .conflict.uncertain { border-color:var(--accent); background:#f0f7ff; }
  .block { border:1px solid var(--line); border-radius:8px; padding:10px 12px; margin:8px 0; }
  .block.budget { border-color:var(--warn); background:#fffdf5; }
  .candidate { font-size:12px; padding:5px 8px; border-radius:6px; margin:4px 0;
              background:#f7fafc; display:flex; gap:10px; align-items:center; flex-wrap:wrap; }
  .candidate.accepted { background:#e6fffa; outline:2px solid var(--ok); }
  button { border:1px solid var(--accent); background:#fff; color:var(--accent);
          border-radius:6px; padding:3px 10px; font-size:12px; cursor:pointer; }
  button:hover { background:#ebf4ff; }
  button.warn { border-color:var(--warn); color:var(--warn); }
  button.danger { border-color:var(--err); color:var(--err); }
  .tag { display:inline-block; padding:1px 7px; border-radius:10px; font-size:11px;
         background:#edf2f7; color:var(--muted); }
  .tag.pending { background:#fff7e6; color:var(--warn); }
  svg.pedigree { width:100%; height:210px; }
  .mono { font-family:ui-monospace,Menlo,monospace; }
  details summary { cursor:pointer; font-size:12px; color:var(--muted); }
  .row { display:flex; gap:8px; align-items:center; margin:6px 0; flex-wrap:wrap; }
  #toast { position:fixed; bottom:18px; right:18px; background:#22303f; color:#fff;
          padding:9px 14px; border-radius:8px; font-size:13px; opacity:0;
          transition:opacity .2s; max-width:380px; }
  #toast.show { opacity:1; }
</style>
</head>
<body>
<header>
  <h1>相位织图</h1>
  <span class="meta">离线家系基因型 / 单倍型分相审阅 · 仅使用本地匿名标识</span>
  <span class="meta" style="margin-left:auto">
    输入版本 <b id="version">-</b> · 分支
    <select id="branch" style="font-size:12px"></select>
  </span>
</header>
<main>
  <div>
    <div class="card">
      <h2>家系图</h2>
      <svg id="pedigree" class="pedigree"></svg>
    </div>
    <div class="card">
      <h2>裁定操作</h2>
      <div class="row">
        <button class="warn" onclick="rollback()">回退最后裁定</button>
        <button onclick="forkBranch()">从当前分支开分支</button>
      </div>
      <div class="row">
        <button onclick="exportData()">导出（钉住版本+事件）</button>
      </div>
      <details><summary>载入演示家系 / 导入 JSON</summary>
        <div class="row"><button onclick="loadDemo()">载入合成演示数据</button></div>
        <textarea id="importJson" rows="6" style="width:100%;font-size:11px"
          placeholder="粘贴 ImportBundle JSON"></textarea>
        <div class="row"><button onclick="importJson()">导入（生成新版本）</button></div>
      </details>
    </div>
    <div class="card">
      <h2>冲突与隔离证据</h2>
      <div id="conflicts"></div>
    </div>
    <div class="card">
      <h2>裁定事件</h2>
      <div id="events" style="font-size:12px"></div>
    </div>
  </div>
  <div>
    <div class="card">
      <h2>变异矩阵</h2>
      <div style="max-height:260px;overflow:auto">
        <table id="matrix"></table>
      </div>
    </div>
    <div class="card">
      <h2>Phase Block 与候选</h2>
      <div id="blocks"></div>
    </div>
  </div>
</main>
<div id="toast"></div>
<script>
"#;

const HTML_PART2: &str = r##"
let STATE = null;

async function refresh() {
  const branch = document.getElementById('branch').value || 'main';
  STATE = await (await fetch('/api/state?branch=' + encodeURIComponent(branch))).json();
  renderAll();
}

function toast(msg) {
  const t = document.getElementById('toast');
  t.textContent = msg; t.classList.add('show');
  setTimeout(() => t.classList.remove('show'), 2600);
}

async function api(path, body, method='POST') {
  const res = await fetch(path, {method,
    headers: {'Content-Type':'application/json'},
    body: body ? JSON.stringify(body) : undefined});
  const data = await res.json();
  if (!res.ok) { toast('错误: ' + (data.error || res.status)); throw data; }
  await refresh();
  return data;
}

function renderAll() {
  document.getElementById('version').textContent = STATE.input_version;
  const sel = document.getElementById('branch');
  if (!STATE.branches.includes(STATE.branch)) STATE.branches.push(STATE.branch);
  sel.innerHTML = STATE.branches.map(b =>
    `<option ${b===STATE.branch?'selected':''}>${b}</option>`).join('');
  renderPedigree(); renderMatrix(); renderBlocks();
  renderConflicts(); renderEvents();
}

function sampleLabel(id) {
  const s = STATE.samples.find(x => x.id === id);
  return s ? s.anon_label : ('#' + id);
}

function renderPedigree() {
  const svg = document.getElementById('pedigree');
  const ids = STATE.samples.map(s => s.id);
  const pos = {};
  ids.forEach((id, i) => pos[id] = {x: 30 + (i % 4) * 75, y: 130 + Math.floor(i / 4) * 60});
  let lines = '';
  for (const r of STATE.relationships) {
    const a = pos[r.parent], b = pos[r.child];
    if (!a || !b) continue;
    const color = r.status === 'to_confirm' ? '#b7791f' : '#2b6cb0';
    const dash = r.status === 'to_confirm' ? 'stroke-dasharray:4 3' : '';
    lines += `<line x1="${a.x}" y1="${a.y}" x2="${b.x}" y2="${b.y}"
      stroke="${color}" stroke-width="2" ${dash}/>`;
    lines += `<text x="${(a.x+b.x)/2+4}" y="${(a.y+b.y)/2-4}" font-size="10" fill="${color}">
      ${r.kind==='father'?'父':'母'}${r.status==='to_confirm'?'?':''}</text>`;
  }
  let nodes = '';
  for (const id of ids) {
    const p = pos[id];
    nodes += `<circle cx="${p.x}" cy="${p.y}" r="13" fill="#fff" stroke="#2b6cb0" stroke-width="2"/>
      <text x="${p.x}" y="${p.y+4}" text-anchor="middle" font-size="11">${sampleLabel(id)}</text>`;
  }
  svg.innerHTML = lines + nodes;
}

function renderMatrix() {
  const variants = STATE.variants;
  let head = '<tr><th>样本</th>' +
    variants.map(v => `<th>${v.chrom}:${v.pos}<br><span class="tag">${v.alleles.join('/')}</span></th>`).join('') + '</tr>';
  let body = '';
  for (const s of STATE.samples) {
    body += `<tr><th>${s.anon_label}</th>`;
    for (const v of variants) {
      const c = STATE.matrix[s.id][v.id];
      const cls = c.missing ? 'miss' : c.ignored ? 'ignored' : c.low_quality_het ? 'lowq' : '';
      body += `<td class="${cls}" title="观测 #${c.observation_id ?? '-'}">${c.genotype}</td>`;
    }
    body += '</tr>';
  }
  document.getElementById('matrix').innerHTML = head + body;
}

function kindClass(k) {
  if (k === 'de_novo_candidate' || k === 'mendelian_inconsistent') return 'denovo';
  if (k === 'uncertain_parentage') return 'uncertain';
  return '';
}
function kindName(k) {
  return {mendelian_inconsistent:'Mendelian 不一致', de_novo_candidate:'de novo 候选',
    missing_genotype:'缺失基因型', low_quality_het:'低质量杂合',
    duplicate_sample:'重复样本', uncertain_parentage:'父母身份不确定'}[k] || k;
}

function renderConflicts() {
  const el = document.getElementById('conflicts');
  if (!STATE.conflicts.length) { el.innerHTML = '<div class="tag">无冲突</div>'; return; }
  el.innerHTML = STATE.conflicts.map(c => `
    <div class="conflict ${kindClass(c.kind)}">
      <b>${kindName(c.kind)}</b>
      ${c.sample!=null?' · '+sampleLabel(c.sample):''}
      ${c.variant!=null?' · 变异#'+c.variant:''}
      <div>${c.detail}</div>
      ${c.minimal_ignore.length ? `<div class="tag">最小隔离观测: ${c.minimal_ignore.join(', ')}</div>`:''}
      ${c.kind==='uncertain_parentage' ?
        `<div style="margin-top:4px"><button onclick="confirmRel(this)" data-rel="${c.detail.match(/关系 (\\d+)/)?.[1]||''}">
          确认此父母关系</button></div>`:''}
    </div>`).join('');
}

async function confirmRel(btn) {
  const id = parseInt(btn.dataset.rel);
  await api('/api/decide', {branch: STATE.branch, expected_version: STATE.input_version,
    decision: {action:'mark_relationship', relationship_id: id, status:'confirmed'}});
}

function orientStr(cand) {
  return cand.orientation.map(o => o ? 'A|a' : 'a|A').join(' — ');
}

function renderBlocks() {
  const el = document.getElementById('blocks');
  if (!STATE.blocks.length) { el.innerHTML = '<div class="tag">尚无可定相 block（先导入数据）</div>'; return; }
  el.innerHTML = STATE.blocks.map(b => `
    <div class="block ${b.budget_hit?'budget':''}">
      <div class="row">
        <b class="mono">${b.id}</b>
        <span class="tag">样本 ${sampleLabel(b.sample)}</span>
        <span class="tag">${b.chrom} : ${b.start_pos}-${b.end_pos}</span>
        <span class="tag">位点 ${b.variant_ids.length}</span>
        ${b.bridge_evidence.length ? `<span class="tag">桥接连段: ${b.bridge_evidence.join(', ')}</span>`:''}
        ${b.budget_hit ? '<span class="tag pending">候选预算已达上限，不保证唯一解</span>':''}
        ${b.candidates.length>1 ? `<span class="tag pending">${b.candidates.length} 个并列候选</span>`:''}
      </div>
      ${b.candidates.map(c => `
        <div class="candidate ${c.accepted?'accepted':''}">
          <span class="mono">#${c.index}</span>
          <span>得分 ${c.score.toFixed(2)}</span>
          <span class="mono">${orientStr(c)}</span>
          <span class="tag">支持 ${c.supporting.length} 项 / 忽略 ${c.ignored.length} 项</span>
          ${!c.accepted ? `<button onclick="accept('${b.id}', ${c.index})">接受</button>` : '<b style="color:#2f855a">已接受</b>'}
          <details><summary>证据明细</summary>
            <div style="margin-top:4px">
              ${c.supporting.map(e=>`<div class="tag">支持: ${e.source==='transmission'?'传递':e.source==='read_link'?'读段':'锁定'} 位点${e.var_a}↔${e.var_b} 权重${e.weight}</div>`).join(' ')}
              ${c.ignored.map(x=>`<div class="tag pending">${x}</div>`).join(' ')}
            </div>
          </details>
        </div>`).join('')}
    </div>`).join('');
}

async function accept(blockId, idx) {
  await api('/api/decide', {branch: STATE.branch, expected_version: STATE.input_version,
    decision: {action:'accept_candidate', block_id: blockId, candidate_index: idx}});
}

function renderEvents() {
  const el = document.getElementById('events');
  if (!STATE.events.length) { el.innerHTML = '<div class="tag">无裁定</div>'; return; }
  el.innerHTML = STATE.events.map(e =>
    `<div>#${e.seq} <span class="tag">v${e.input_version}</span> ${JSON.stringify(e.decision)}</div>`
  ).join('');
}

async function rollback() {
  await api('/api/rollback', {branch: STATE.branch});
  toast('已回退最后一条裁定');
}

async function forkBranch() {
  const name = prompt('新分支名称', 'review-' + Date.now());
  if (!name) return;
  await api('/api/fork', {from: STATE.branch, to: name});
  document.getElementById('branch').value = name;
  await refresh();
}

async function exportData() {
  const data = await (await fetch('/api/export?branch=' + encodeURIComponent(STATE.branch))).json();
  const blob = new Blob([JSON.stringify(data, null, 2)], {type:'application/json'});
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'phase-weave-export-v' + data.input_version + '.json';
  a.click();
}

async function importJson() {
  const txt = document.getElementById('importJson').value;
  if (!txt.trim()) return;
  const data = await api('/api/import', JSON.parse(txt));
  toast('导入完成，输入版本 v' + data.input_version);
}

function demoBundle() {
  return {
    samples: [
      {id:1, anon_label:'S-A1F', batch:'batch-001'},
      {id:2, anon_label:'S-A1M', batch:'batch-001'},
      {id:3, anon_label:'S-A1C', batch:'batch-002'}],
    relationships: [
      {id:11, child:3, parent:1, kind:'father', status:'confirmed'},
      {id:12, child:3, parent:2, kind:'mother', status:'confirmed'}],
    variants: [
      {id:101, chrom:'chr1', pos:1000, alleles:['A','G']},
      {id:102, chrom:'chr1', pos:2000, alleles:['C','T']},
      {id:103, chrom:'chr1', pos:3000, alleles:['G','T']}],
    observations: [
      // father hom A,A hom C,C
      {id:201, sample:1, variant:101, gls:[0.99,0.009,0.001], quality:99, batch:'b1'},
      {id:202, sample:1, variant:102, gls:[0.98,0.015,0.005], quality:90, batch:'b1'},
      // mother het at both
      {id:203, sample:2, variant:101, gls:[0.05,0.92,0.03], quality:80, batch:'b1'},
      {id:204, sample:2, variant:102, gls:[0.04,0.91,0.05], quality:80, batch:'b1'},
      // child het at both -> paternal allele known, phased
      {id:205, sample:3, variant:101, gls:[0.02,0.96,0.02], quality:95, batch:'b2'},
      {id:206, sample:3, variant:102, gls:[0.03,0.93,0.04], quality:88, batch:'b2'},
      // triallelic error site: father T,T mother T,T child G,G -> conflict
      {id:207, sample:1, variant:103, gls:[0.01,0.01,0.01,0.01,0.01,0.95], quality:90, batch:'b1'},
      {id:208, sample:2, variant:103, gls:[0.01,0.01,0.01,0.01,0.01,0.95], quality:90, batch:'b1'},
      {id:209, sample:3, variant:103, gls:[0.95,0.01,0.01,0.01,0.01,0.01], quality:60, batch:'b2'}],
    read_links: []
  };
}

async function loadDemo() {
  document.getElementById('importJson').value = JSON.stringify(demoBundle(), null, 2);
  await importJson();
}

document.getElementById('branch').addEventListener('change', refresh);
refresh();
</script>
</body>
</html>"##;

pub fn index_html() -> String {
    let mut page = String::with_capacity(HTML_PART1.len() + HTML_PART2.len());
    page.push_str(HTML_PART1);
    page.push_str(HTML_PART2);
    page
}
