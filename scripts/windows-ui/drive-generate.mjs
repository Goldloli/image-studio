// Acceptance-only driver: run one real generation through the installed app's UI.
// usage: node scripts/windows-ui/drive-generate.mjs --model <substring> --kind <text|edit|remove-background|upscale>
//        [--prompt <text>] [--source <prompt-substring>] [--timeout 1800] [--out json] [--shot png]
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
  const kind = arg("kind", "text");
  const prompt = arg("prompt", "");
  const source = arg("source", "");
  const timeoutSeconds = Number(arg("timeout", "1800"));
  const cancelAfter = Number(arg("cancel-after", "0"));
  const out = arg("out", "generate-result.json");
  const shotPath = arg("shot", "");
  const started = Date.now();
  const record = { model, kind, prompt, source, startedAt: new Date().toISOString(), status: "unknown", stages: [] };
  const cdp = await connect();
  const evaluate = async (expression) => {
    const result = await cdp.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    return result.result?.value;
  };
  const waitFor = async (expression, timeoutMs, intervalMs = 300) => {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const value = await evaluate(expression);
      if (value) return value;
      await sleep(intervalMs);
    }
    return null;
  };
  const shot = async () => {
    if (!shotPath) return;
    const result = await cdp.send("Page.captureScreenshot", { format: "png" });
    const fs = await import("node:fs");
    fs.mkdirSync((await import("node:path")).dirname(shotPath), { recursive: true });
    fs.writeFileSync(shotPath, Buffer.from(result.data, "base64"));
  };
  try {
    await evaluate(`(()=>{const b=document.querySelector('.primary-navigation button[aria-label="创作"]');if(b)b.click();return !!b})()`);
    const composer = await waitFor(`!!document.querySelector('.conversation-composer')`, 15000);
    if (!composer) throw new Error("composer not available on create page");
    // 1. select model through the real composer popover
    await evaluate(`(()=>{document.querySelector('.model-select')?.click();return true})()`);
    const options = await waitFor(`(()=>{const p=document.querySelector('.composer-popover.model-popover');if(!p)return null;return [...p.querySelectorAll('.single-model-option')].map(b=>({text:b.querySelector('strong')?.textContent||'',disabled:b.disabled}))})()`, 8000);
    if (!options) throw new Error("model popover did not open");
    record.availableModels = options.map((option) => option.text);
    const pick = await evaluate(`(()=>{
      const p=document.querySelector('.composer-popover.model-popover');
      if(!p) return {clicked:false,reason:'popover missing'};
      const options=[...p.querySelectorAll('.single-model-option')].filter(b=>!b.disabled);
      const target=options.find(b=>(b.querySelector('strong')?.textContent||'').includes(${JSON.stringify(model)}));
      if(!target) return {clicked:false,options:options.map(b=>b.querySelector('strong')?.textContent||'')};
      target.click();return {clicked:true,text:target.querySelector('strong')?.textContent||''};
    })()`);
    if (!pick || !pick.clicked) throw new Error(`model option not found for ${model}: ${JSON.stringify(pick)}`);
    record.selectedModel = pick.text;
    await sleep(400);
    // 2. for edit/tool capabilities choose the source work from the work library
    if (kind !== "text") {
      await evaluate(`(()=>{const chip=[...document.querySelectorAll('.composer-chip')].find(b=>b.textContent.includes('作品'));if(chip)chip.click();return !!chip})()`);
      const sources = await waitFor(`(()=>{const p=document.querySelector('.source-popover');if(!p)return null;return [...p.querySelectorAll('.single-model-option')].map(b=>({text:b.querySelector('small')?.textContent||''}))})()`, 8000);
      if (!sources) throw new Error("source popover did not open");
      const choice = await evaluate(`(()=>{
        const p=document.querySelector('.source-popover');
        if(!p) return {clicked:false,reason:'source popover missing'};
        const options=[...p.querySelectorAll('.single-model-option')];
        const target=options.find(b=>(b.querySelector('small')?.textContent||'').includes(${JSON.stringify(source)}));
        if(!target) return {clicked:false,options:options.map(b=>b.querySelector('small')?.textContent||'')};
        target.click();return {clicked:true};
      })()`);
      if (!choice || !choice.clicked) throw new Error(`source work not found for "${source}": ${JSON.stringify(choice)}`);
      await sleep(500);
      // selecting a work for a tool or Klein edit capability switches the capability immediately
    }
    // 3. type the prompt for text/edit capabilities
    if (prompt) {
      const typed = await evaluate(`(()=>{const el=document.querySelector('.conversation-composer textarea');if(!el)return 'missing';el.focus();const setter=Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set;setter.call(el,${JSON.stringify(prompt)});el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true}));return 'typed';})()`);
      if (typed !== "typed") throw new Error("prompt textarea not found");
      await sleep(300);
    }
    // 4. send
    const before = await evaluate(`(()=>({turns:document.querySelectorAll('article.conversation-turn').length,sent:[...document.querySelectorAll('button.composer-send')].length}))()`);
    record.turnsBefore = before;
    const sent = await evaluate(`(()=>{const b=document.querySelector('button.composer-send');if(!b)return {clicked:false};if(b.disabled)return {clicked:false,disabled:true};b.click();return {clicked:true};})()`);
    if (!sent || !sent.clicked) throw new Error(`send button unavailable: ${JSON.stringify(sent)}`);
    const emittedAt = Date.now();
    record.sendAt = new Date().toISOString();
    // 5. poll the last turn until it is terminal
    let lastText = "";
    let stages = [];
    const deadline = Date.now() + timeoutSeconds * 1000;
    let status = "unknown";
    const TERMINAL = ["Succeeded", "Failed", "Cancelled", "Interrupted", "Rejected"];
    let cancelClickedAt = null;
    while (Date.now() < deadline) {
      await sleep(1500);
      if (cancelAfter > 0 && !cancelClickedAt && (Date.now() - emittedAt) >= cancelAfter * 1000) {
        const cancelResult = await evaluate(`(()=>{var b=document.querySelector('button.composer-send.is-stop');if(!b)return {clicked:false};b.click();return {clicked:true}})()`);
        if (cancelResult && cancelResult.clicked) {
          cancelClickedAt = Date.now();
          record.cancelClickedAt = new Date().toISOString();
        }
      }
      const state = await evaluate(`(()=>{
        const turns=[...document.querySelectorAll('article.conversation-turn')];
        const last=turns[turns.length-1];
        const text=last?last.innerText:'';
        const cls=last?String(last.className):'';
        const m=cls.match(/state-(\\w+)/);
        const active=document.querySelector('button.composer-send.is-stop');
        return {turns:turns.length,text,turnState:m?m[1]:'',active:!!active,turnId:last?last.getAttribute('data-turn-id'):null};
      })()`);
      lastText = state.text || "";
      const stageMatch = lastText.match(/(正在[^\n]{1,40}|采样[^\n]{0,20}|正在保存[^\n]{0,20}|排队[^\n]{0,20})/);
      const stage = stageMatch ? stageMatch[0] : "";
      if (stage && (!stages.length || stages[stages.length - 1].stage !== stage)) {
        stages.push({ at: new Date().toISOString(), elapsedSeconds: Math.round((Date.now() - emittedAt) / 1000), stage });
      }
      record.stages = stages;
      if (state.turnState && TERMINAL.includes(state.turnState)) {
        record.turnId = state.turnId;
        record.terminalState = state.turnState;
        record.settleSecondsAfterCancel = cancelClickedAt ? Math.round((Date.now() - cancelClickedAt) / 100) / 10 : null;
        if (state.turnState === "Succeeded") { status = "succeeded"; break; }
        if (state.turnState === "Cancelled" && cancelClickedAt) { status = "cancelled"; break; }
        status = "failed";
        record.error = lastText.slice(0, 1000);
        break;
      }
      // Fallback text detection for any turn state that the class does not expose.
      if (/图片已生成|生成完成/.test(lastText) && !state.active) { status = "succeeded"; record.turnId = state.turnId; break; }
      if (/失败|已取消|已中断|超出资源预算/.test(lastText) && !state.active) { status = "failed"; record.turnId = state.turnId; record.error = lastText.slice(0, 1000); break; }
    }
    record.status = status === "unknown" ? "timeout" : status;
    record.stages = stages;
    record.turnText = lastText.slice(0, 1200);
    record.elapsedSeconds = Math.round((Date.now() - emittedAt) / 1000);
    await shot();
    record.endedAt = new Date().toISOString();
  } catch (error) {
    record.status = "error";
    record.error = String(error && error.message ? error.message : error);
    try { await shot(); } catch { }
  } finally {
    const fs = await import("node:fs");
    fs.writeFileSync(out, JSON.stringify(record, null, 2));
    cdp.close();
    console.log(JSON.stringify({ model, kind, status: record.status, elapsedSeconds: record.elapsedSeconds, error: record.error }, null, 2));
  }
  process.exit(record.status === "succeeded" ? 0 : 1);
}

main().catch((error) => { console.error(error); process.exit(2); });
