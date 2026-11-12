// Acceptance-only driver: install one model through the installed app's real UI.
// usage: node scripts/windows-ui/drive-install.mjs --model <name-substring> --timeout 2400 --out <json> [--shot-dir <dir>]
const PORT = Number(process.env.CDP_PORT || 9222);

function arg(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback;
}

async function connect() {
  const res = await fetch(`http://127.0.0.1:${PORT}/json`);
  const targets = await res.json();
  const page = targets.find((target) => target.type === "page");
  if (!page) throw new Error("no page target");
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    const pending = new Map();
    let nextId = 1;
    ws.onopen = () => resolve({
      send(method, params = {}) {
        const id = nextId++;
        return new Promise((res, rej) => {
          pending.set(id, { res, rej });
          ws.send(JSON.stringify({ id, method, params }));
        });
      },
      close() { ws.close(); },
    });
    ws.onerror = () => reject(new Error("websocket error"));
    ws.onmessage = (event) => {
      const message = JSON.parse(event.data);
      if (message.id && pending.has(message.id)) {
        const { res, rej } = pending.get(message.id);
        pending.delete(message.id);
        if (message.error) rej(new Error(JSON.stringify(message.error)));
        else res(message.result);
      }
    };
  });
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function main() {
  const model = arg("model", "Klein");
  const timeoutSeconds = Number(arg("timeout", "2400"));
  const out = arg("out", "install-result.json");
  const shotDir = arg("shot-dir", "");
  const started = Date.now();
  const cdp = await connect();
  const evaluate = async (expression) => {
    const result = await cdp.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    return result.result?.value;
  };
  const shot = async (name) => {
    if (!shotDir) return;
    const result = await cdp.send("Page.captureScreenshot", { format: "png" });
    const fs = await import("node:fs");
    fs.mkdirSync(shotDir, { recursive: true });
    fs.writeFileSync(`${shotDir}/${name}.png`, Buffer.from(result.data, "base64"));
  };
  const phases = [];
  const record = { model, startedAt: new Date().toISOString(), phases, status: "unknown" };
  try {
    await evaluate(`(()=>{const b=document.querySelector('.primary-navigation button[aria-label="模型"]');if(b)b.click();return !!b})()`);
    // wait for the catalog to render
    let found = false;
    for (let i = 0; i < 60; i++) {
      const state = await evaluate(`(()=>{const rows=[...document.querySelectorAll('article.catalog-model')];const target=rows.find(r=>{const n=r.querySelector('.catalog-model-name strong');return n&&n.textContent.includes(${JSON.stringify(model)})});if(!target)return {found:false,count:rows.length};target.querySelector('.catalog-model-row')?.click();return {found:true};})()`);
      if (state && state.found) { found = true; break; }
      await sleep(500);
    }
    if (!found) throw new Error(`model row not found: ${model}`);
    await sleep(400);
    const snapshot = async () => evaluate(`(()=>{
      const rows=[...document.querySelectorAll('article.catalog-model')];
      const target=rows.find(r=>{const n=r.querySelector('.catalog-model-name strong');return n&&n.textContent.includes(${JSON.stringify(model)})});
      if(!target) return {found:false};
      const status=(target.querySelector('.catalog-status')?.textContent||'').trim();
      const alert=(target.querySelector('[role=alert]')?.textContent||'').trim();
      const buttons=[...target.querySelectorAll('.catalog-actions button')].map(b=>({text:b.textContent.trim(),disabled:b.disabled}));
      const progress=(target.querySelector('.catalog-progress')?.textContent||'').trim();
      return {found:true,status,alert,buttons,progress};
    })()`);
    let state = await snapshot();
    if (!state || !state.found) throw new Error("catalog row disappeared");
    const clickInstall = async () => evaluate(`(()=>{
      const rows=[...document.querySelectorAll('article.catalog-model')];
      const target=rows.find(r=>{const n=r.querySelector('.catalog-model-name strong');return n&&n.textContent.includes(${JSON.stringify(model)})});
      if(!target) return {clicked:false,reason:'row missing'};
      const row=target.querySelector('.catalog-model-row');
      if(row && !target.classList.contains('is-open')) row.click();
      const buttons=[...target.querySelectorAll('.catalog-actions button')];
      const target_button=buttons.find(b=>!b.disabled && /安装|继续安装|修复安装/.test(b.textContent));
      if(!target_button) return {clicked:false,buttons:buttons.map(b=>b.textContent.trim())};
      target_button.click();
      return {clicked:true,text:target_button.textContent.trim()};
    })()`);
    if (/当前可执行|模型已就绪/.test(state.status)) {
      record.status = "already_ready";
    } else {
      let clickResult = await clickInstall();
      if (!clickResult.clicked) {
        // expand explicitly and retry once
        await evaluate(`(()=>{const rows=[...document.querySelectorAll('article.catalog-model')];const target=rows.find(r=>{const n=r.querySelector('.catalog-model-name strong');return n&&n.textContent.includes(${JSON.stringify(model)})});target?.querySelector('.catalog-model-row')?.click();return true})()`);
        await sleep(500);
        clickResult = await clickInstall();
      }
      if (!clickResult.clicked) throw new Error(`install button not found for ${model}: ${JSON.stringify(clickResult)}`);
      record.clicked = clickResult;
      let lastStatus = "";
      const deadline = Date.now() + timeoutSeconds * 1000;
      let shotIndex = 0;
      let pollIndex = 0;
      while (Date.now() < deadline) {
        await sleep(2000);
        pollIndex += 1;
        // The model page auto-poll can stall after an install finishes; force a
        // catalog refresh every 10 s so the driver observes the real state.
        if (pollIndex % 5 === 0) {
          await evaluate(`(()=>{const b=[...document.querySelectorAll('.model-heading button')].find(x=>x.textContent.includes('刷新'));if(b)b.click();return !!b})()`);
        }
        state = await snapshot();
        if (!state || !state.found) throw new Error("catalog row disappeared during install");
        if (state.status !== lastStatus) {
          lastStatus = state.status;
          phases.push({ at: new Date().toISOString(), elapsedSeconds: Math.round((Date.now() - started) / 1000), status: state.status, progress: state.progress });
          await shot(`install-${String(shotIndex++).padStart(2, "0")}-${state.status.replace(/[^\w\u4e00-\u9fa5-]+/g, "_").slice(0, 30)}`);
        }
        if (/当前可执行|模型已就绪/.test(state.status)) { record.status = "ready"; break; }
        if (/安装失败|安装文件缺失或身份异常/.test(state.status) || (state.alert && /失败|error/i.test(state.alert) && !/可修复/.test(state.alert))) {
          record.status = "failed";
          record.error = state.alert || state.status;
          break;
        }
        if (/设备不满足门槛/.test(state.status)) { record.status = "blocked"; record.error = state.status; break; }
      }
      if (record.status === "unknown") { record.status = "timeout"; record.lastStatus = state.status; }
    }
    await shot("install-final");
    record.endedAt = new Date().toISOString();
    record.elapsedSeconds = Math.round((Date.now() - started) / 1000);
    record.final = state;
  } catch (error) {
    record.status = "error";
    record.error = String(error && error.message ? error.message : error);
  } finally {
    const fs = await import("node:fs");
    fs.writeFileSync(out, JSON.stringify(record, null, 2));
    cdp.close();
    console.log(JSON.stringify({ model, status: record.status, elapsedSeconds: record.elapsedSeconds, error: record.error }, null, 2));
  }
  process.exit(record.status === "ready" || record.status === "already_ready" ? 0 : 1);
}

main().catch((error) => { console.error(error); process.exit(2); });
