// Acceptance-only driver for the works page, preview/export, references and conversations.
// usage: node scripts/windows-ui/drive-works.mjs --action <name> [--tag <substr>] [--path <file>]
//        [--title <substr>] [--new-name <text>] [--label <substr>] [--index <n>] [--shot <png>] [--out <json>]
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

const CARDS_JS = "[...document.querySelectorAll('.works-grid .work-card, .works-list .work-card')].map(function(c){return {prompt:(c.querySelector('.work-card-body strong')||{}).textContent||'',conv:(c.querySelector('.work-card-body div span')||{}).textContent||'',meta:(c.querySelector('.work-card-body p')||{}).textContent||'',favorite:!!(c.querySelector('.work-favorite')&&c.querySelector('.work-favorite').classList.contains('is-favorite')),managing:c.classList.contains('is-managing'),status:(c.querySelector('.work-card-actions span')||{}).textContent||'',deletionState:(c.querySelector('.work-deletion-state')||{}).textContent||''}})";

function findCardJs(tag) {
  return "(()=>{var cards=[...document.querySelectorAll('.works-grid .work-card, .works-list .work-card')];return cards.find(function(c){var n=c.querySelector('.work-card-body strong');return n&&n.textContent.includes(" + JSON.stringify(tag) + ")})||null})()";
}

// Build an expression that selects the nth card matching a prompt tag or a meta substring.
function cardSelector(tag, nth, meta) {
  return "var cards=[...document.querySelectorAll('.works-grid .work-card, .works-list .work-card')].filter(function(c){" +
    "var n=c.querySelector('.work-card-body strong');var m=c.querySelector('.work-card-body p');" +
    "var okPrompt=" + (tag ? "!!n&&n.textContent.includes(" + JSON.stringify(tag) + ")" : "false") + ";" +
    "var okMeta=" + (meta ? "!!m&&m.textContent.includes(" + JSON.stringify(meta) + ")" : "false") + ";" +
    "return okPrompt||okMeta});" +
    "var card=cards[" + nth + "]||null;";
}

async function main() {
  const action = arg("action", "list");
  const tag = arg("tag", "");
  const filePath = arg("path", "");
  const title = arg("title", "");
  const newName = arg("new-name", "");
  const label = arg("label", "");
  const nth = Number(arg("nth", "0"));
  const metaMatch = arg("meta", "");
  const selectIndex = Number(arg("index", "0"));
  const shotPath = arg("shot", "");
  const outPath = arg("out", "");
  const cdp = await connect();
  const ev = async (expression) => {
    const result = await cdp.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description || "eval error");
    return result.result?.value;
  };
  const waitFor = async (expression, timeoutMs, intervalMs = 250) => {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const value = await ev(expression);
      if (value) return value;
      await sleep(intervalMs);
    }
    return null;
  };
  const shot = async () => {
    if (!shotPath) return;
    const result = await cdp.send("Page.captureScreenshot", { format: "png" });
    const fs = await import("node:fs");
    const path = await import("node:path");
    fs.mkdirSync(path.dirname(shotPath), { recursive: true });
    fs.writeFileSync(shotPath, Buffer.from(result.data, "base64"));
  };
  const nav = async (index) => {
    await ev(`(()=>{var b=document.querySelectorAll('.primary-navigation button')[${index}];if(b)b.click();return !!b})()`);
    await sleep(index === 3 ? 900 : 500);
  };
  const navCreate = () => nav(0);
  const navWorks = () => nav(3);
  const cards = () => ev(CARDS_JS);
  const listState = () => ev("(()=>({countLabel:(document.querySelector('.works-view-row span')||{}).textContent||'',countRow:(document.querySelector('.works-view-row')||{}).textContent||'',empty:(document.querySelector('.library-empty h2')||{}).textContent||'',error:(document.querySelector('.library-error')||{}).textContent||''}))()");
  let result = { action };
  try {
    if (action === "text") {
      result.text = await ev("document.body.innerText");
    } else if (action === "list") {
      await navWorks();
      const state = await listState();
      const items = await cards();
      result = { action, state, items };
    } else if (action === "favorite") {
      await navWorks();
      result = await ev("(()=>{" + cardSelector(tag, nth, metaMatch) + "if(!card)return {clicked:false,reason:'card missing'};var b=card.querySelector('.work-favorite');var before=b.classList.contains('is-favorite');b.click();return new Promise(function(resolve){setTimeout(function(){resolve({clicked:true,before:before,after:card.querySelector('.work-favorite').classList.contains('is-favorite')})},1200)})})()");
    } else if (action === "favorites-filter") {
      await navWorks();
      const value = arg("value", "on");
      result = await ev("(()=>{var buttons=[...document.querySelectorAll('.works-favorite-filter button')];var t=buttons[" + (value === "on" ? 1 : 0) + "];if(!t)return {clicked:false};t.click();return new Promise(function(resolve){setTimeout(function(){resolve({clicked:true,active:[...document.querySelectorAll('.works-favorite-filter button')].map(function(b){return {text:b.textContent.trim(),active:b.classList.contains('active')}}),state:(function(){var w=document.querySelector('.works-view-row');return {count:(w||{}).textContent||''}})()})},1200)})})()");
      result.items = await cards();
    } else if (action === "clear-filters") {
      await navWorks();
      result = await ev("(()=>{var b=document.querySelector('.filter-clear');if(!b)return {clicked:false};b.click();return {clicked:true}})()");
      await sleep(1200);
      result.items = await cards();
      result.state = await listState();
    } else if (action === "filter-select") {
      await navWorks();
      result = await ev("(()=>{var selects=[...document.querySelectorAll('.works-filter-row select')];var s=selects[" + selectIndex + "];if(!s)return {found:false,count:selects.length};var opts=[...s.options].map(function(o){return {value:o.value,text:o.textContent}});var opt=opts.find(function(o){return o.text.includes(" + JSON.stringify(label) + ")});if(!opt)return {found:false,options:opts};var setter=Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set;setter.call(s,opt.value);s.dispatchEvent(new Event('change',{bubbles:true}));return {found:true,option:opt}})()");
      await sleep(1500);
      result.items = await cards();
      result.state = await listState();
    } else if (action === "preview") {
      await navWorks();
      const opened = await ev("(()=>{" + cardSelector(tag, nth, metaMatch) + "if(!card)return {opened:false};var img=card.querySelector('.work-card-image');if(img)img.click();return {opened:true}})()");
      await waitFor("!!document.querySelector('.works-preview')", 5000);
      await sleep(700);
      const preview = await ev("(()=>{var p=document.querySelector('.works-preview');if(!p)return null;var dl=[...p.querySelectorAll('dt')].map(function(dt){return dt.textContent+'='+((dt.nextElementSibling||{}).textContent||'')});var ex=p.querySelector('.preview-actions .primary-action');return {text:p.innerText.slice(0,1500),facts:dl,imageCount:p.querySelectorAll('img').length,exportDisabled:ex?ex.disabled:null}})()");
      result = { action, opened, preview };
      await shot();
    } else if (action === "close-preview") {
      result = await ev("(()=>{var b=document.querySelector('[aria-label=\"关闭作品预览\"]');if(!b)return {clicked:false};b.click();return {clicked:true}})()");
      await sleep(400);
    } else if (action === "export") {
      const clicked = await ev("(()=>{var b=document.querySelector('.works-preview .preview-actions .primary-action');if(!b)return {clicked:false,reason:'missing'};if(b.disabled)return {clicked:false,reason:'disabled'};b.click();return {clicked:true}})()");
      if (!clicked.clicked) throw new Error("export click failed: " + JSON.stringify(clicked));
      const { execFileSync } = await import("node:child_process");
      const dialog = execFileSync("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts\\windows-ui\\save-ui-dialog.ps1", "-Path", filePath], { encoding: "utf8", timeout: 60000 });
      let message = await waitFor("(()=>{var m=document.querySelector('.works-preview .export-message');return m?m.textContent:null})()", 20000, 500);
      if (!message) message = await ev("(()=>{var m=document.querySelector('.works-preview .export-message');return m?m.textContent:''})()");
      result = { action, clicked, dialog: dialog.trim(), message };
      await sleep(400);
      await shot();
    } else if (action === "delete") {
      await navWorks();
      const clicked = await ev("(()=>{" + cardSelector(tag, nth, metaMatch) + "if(!card)return {clicked:false};var b=card.querySelector('.work-delete');if(b)b.click();return {clicked:true}})()");
      await waitFor("!!document.querySelector('.confirm-dialog')", 5000);
      await sleep(300);
      const dialog = await ev("(()=>{var d=document.querySelector('.confirm-dialog');if(!d)return null;var t=d.querySelector('h2'),desc=d.querySelector('.confirm-description'),err=d.querySelector('.confirm-error');return {title:t?t.textContent:'',description:desc?desc.textContent:'',error:err?err.textContent:''}})()");
      result = { action, clicked, dialog };
      result.items = await cards();
      await shot();
    } else if (action === "confirm-delete" || action === "confirm-dialog") {
      const clicked = await ev("(()=>{var b=document.querySelector('.confirm-dialog .danger');if(!b)return {clicked:false};if(b.disabled)return {clicked:false,disabled:true};b.click();return {clicked:true}})()");
      await sleep(2500);
      const dialog = await ev("(()=>{var d=document.querySelector('.confirm-dialog');if(!d)return null;var err=d.querySelector('.confirm-error'),t=d.querySelector('h2');return {title:t?t.textContent:'',error:err?err.textContent:''}})()");
      result = { action, clicked, dialog };
      result.items = await cards();
      result.bodyError = await ev("(()=>{var e=document.querySelector('.library-error');return e?e.textContent:''})()");
      await shot();
    } else if (action === "cancel-dialog") {
      result = await ev("(()=>{var b=document.querySelector('.confirm-dialog .secondary');if(!b)return {clicked:false};b.click();return {clicked:true}})()");
      await sleep(500);
      result.dialogPresent = await ev("!!document.querySelector('.confirm-dialog')");
    } else if (action === "source-set") {
      await navCreate();
      await ev("(()=>{var b=document.querySelector('.model-select');if(b)b.click();return !!b})()");
      await waitFor("!!document.querySelector('.composer-popover.model-popover')", 5000);
      const picked = await ev("(()=>{var p=document.querySelector('.composer-popover.model-popover');if(!p)return {clicked:false};var o=[...p.querySelectorAll('.single-model-option')].find(function(b){return !b.disabled&&(b.querySelector('strong')||{}).textContent&&b.querySelector('strong').textContent.includes('Klein')});if(!o)return {clicked:false};o.click();return {clicked:true,text:o.querySelector('strong').textContent}})()");
      await sleep(500);
      await ev("(()=>{var chip=[...document.querySelectorAll('.composer-chip')].find(function(b){return b.textContent.includes('作品')});if(chip)chip.click();return !!chip})()");
      await waitFor("!!document.querySelector('.source-popover')", 5000);
      const chosen = await ev("(()=>{var p=document.querySelector('.source-popover');if(!p)return {clicked:false};var o=[...p.querySelectorAll('.single-model-option')].find(function(b){var s=b.querySelector('small');return s&&s.textContent.includes(" + JSON.stringify(tag) + ")});if(!o)return {clicked:false,options:[...p.querySelectorAll('small')].map(function(s){return s.textContent})};o.click();return {clicked:true}})()");
      await sleep(700);
      const pill = await ev("(()=>{var p=document.querySelector('.source-pill');return p?p.textContent:''})()");
      const caption = await ev("(()=>{var c=document.querySelector('.composer-caption');return c?c.textContent:''})()");
      result = { action, picked, chosen, pill, caption };
      await shot();
    } else if (action === "source-clear") {
      result = await ev("(()=>{var b=document.querySelector('[aria-label=\"移除源图\"]');if(!b)return {clicked:false};b.click();return {clicked:true}})()");
      await sleep(500);
      result.pill = await ev("(()=>{var p=document.querySelector('.source-pill');return p?p.textContent:''})()");
    } else if (action === "conversations") {
      await navCreate();
      result = await ev("(()=>[...document.querySelectorAll('.conversation-list-item')].map(function(item){var open=item.querySelector('.conversation-open span');return {id:(item.querySelector('.conversation-delete')||{}).id||'',title:open?open.textContent:'',active:item.classList.contains('is-active'),running:!!item.querySelector('.running-dot')}}))()");
    } else if (action === "conversation-new") {
      await navCreate();
      result = await ev("(()=>{var b=document.querySelector('.new-conversation');if(!b)return {clicked:false};b.click();return {clicked:true}})()");
      await sleep(1200);
      result.conversations = await ev("(()=>[...document.querySelectorAll('.conversation-list-item')].map(function(i){var s=i.querySelector('.conversation-open span');return {title:s?s.textContent:'',active:i.classList.contains('is-active')}}))()");
    } else if (action === "conversation-switch") {
      await navCreate();
      result = await ev("(()=>{var item=[...document.querySelectorAll('.conversation-list-item')].find(function(i){var s=i.querySelector('.conversation-open span');return s&&s.textContent.includes(" + JSON.stringify(title) + ")});if(!item)return {clicked:false};item.querySelector('.conversation-open').click();return {clicked:true}})()");
      await sleep(1200);
      result.active = await ev("(()=>{var a=document.querySelector('.conversation-list-item.is-active .conversation-open span');return a?a.textContent:''})()");
    } else if (action === "conversation-rename") {
      await navCreate();
      result = await ev("(()=>{var item=[...document.querySelectorAll('.conversation-list-item')].find(function(i){var s=i.querySelector('.conversation-open span');return s&&s.textContent.includes(" + JSON.stringify(title) + ")});if(!item)return {clicked:false};var b=item.querySelector('.conversation-rename');if(!b)return {clicked:false};b.click();return {clicked:true}})()");
      const hasInput = await waitFor("!!document.querySelector('input[aria-label=\"会话名称\"]')", 4000);
      if (hasInput) {
        await ev("(()=>{var el=document.querySelector('input[aria-label=\"会话名称\"]');var setter=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;setter.call(el," + JSON.stringify(newName) + ");el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));return true})()");
        await sleep(1200);
      }
      result.titles = await ev("(()=>[...document.querySelectorAll('.conversation-open span')].map(function(s){return s.textContent}))()");
    } else if (action === "conversation-delete") {
      await navCreate();
      result = await ev("(()=>{var item=[...document.querySelectorAll('.conversation-list-item')].find(function(i){var s=i.querySelector('.conversation-open span');return s&&s.textContent.includes(" + JSON.stringify(title) + ")});if(!item)return {clicked:false};var b=item.querySelector('.conversation-delete');if(!b)return {clicked:false,reason:'no button'};if(b.disabled)return {clicked:false,reason:'disabled'};b.click();return {clicked:true}})()");
      await waitFor("!!document.querySelector('.confirm-dialog')", 5000);
      await sleep(300);
      result.dialog = await ev("(()=>{var d=document.querySelector('.confirm-dialog');if(!d)return null;var t=d.querySelector('h2'),desc=d.querySelector('.confirm-description'),err=d.querySelector('.confirm-error');return {title:t?t.textContent:'',description:desc?desc.textContent:'',error:err?err.textContent:''}})()");
      await shot();
    } else if (action === "conversation-confirm-delete") {
      const clicked = await ev("(()=>{var b=document.querySelector('.confirm-dialog .danger');if(!b)return {clicked:false};if(b.disabled)return {clicked:false,disabled:true};b.click();return {clicked:true}})()");
      await sleep(2500);
      result = { action, clicked };
      result.dialogPresent = await ev("!!document.querySelector('.confirm-dialog')");
      result.dialogError = await ev("(()=>{var e=document.querySelector('.confirm-error');return e?e.textContent:''})()");
      result.titles = await ev("(()=>[...document.querySelectorAll('.conversation-open span')].map(function(s){return s.textContent}))()");
      result.actionError = await ev("(()=>{var e=document.querySelector('.conversation-action-error');return e?e.textContent:''})()");
      await shot();
    } else if (action === "shot") {
      await shot();
    } else {
      throw new Error("unknown action: " + action);
    }
  } catch (error) {
    result.error = String(error && error.message ? error.message : error);
  } finally {
    const fs = await import("node:fs");
    if (outPath) fs.writeFileSync(outPath, JSON.stringify(result, null, 2));
    cdp.close();
    console.log(JSON.stringify(result).slice(0, 4000));
  }
  process.exit(result.error ? 1 : 0);
}

main().catch((error) => { console.error(error); process.exit(2); });
