// Minimal CDP client for the installed Image Studio WebView2 page.
// usage: node cdp.mjs <command> [args]
//   eval "<expr>"      evaluate expression in page, print JSON result
//   text               print document.body.innerText
//   html <selector>    print outerHTML of first match (or body)
//   shot <path>        capture PNG screenshot
//   click <selector>   click first matching element
//   type <selector> <text>  focus element, set value through the native setter and fire input
//   wait <ms>          sleep
const PORT = Number(process.env.CDP_PORT || 9333);

async function pageTarget() {
  const res = await fetch(`http://127.0.0.1:${PORT}/json`);
  const targets = await res.json();
  const page = targets.find((t) => t.type === "page");
  if (!page) throw new Error("no page target");
  return page;
}

function connect(url) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    const pending = new Map();
    let nextId = 1;
    ws.onopen = () =>
      resolve({
        send(method, params = {}) {
          const id = nextId++;
          return new Promise((res, rej) => {
            pending.set(id, { res, rej });
            ws.send(JSON.stringify({ id, method, params }));
          });
        },
        close() {
          ws.close();
        },
      });
    ws.onerror = (e) => reject(new Error("websocket error"));
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

async function main() {
  const [command, ...args] = process.argv.slice(2);
  const target = await pageTarget();
  const cdp = await connect(target.webSocketDebuggerUrl);
  try {
    if (command === "eval") {
      const result = await cdp.send("Runtime.evaluate", {
        expression: args[0],
        returnByValue: true,
        awaitPromise: true,
      });
      console.log(JSON.stringify(result.result?.value ?? result, null, 2));
    } else if (command === "text") {
      const result = await cdp.send("Runtime.evaluate", {
        expression: "document.body.innerText",
        returnByValue: true,
      });
      console.log(result.result.value);
    } else if (command === "html") {
      const selector = args[0] || "body";
      const result = await cdp.send("Runtime.evaluate", {
        expression: `(document.querySelector(${JSON.stringify(selector)})||document.body).outerHTML`,
        returnByValue: true,
      });
      console.log(result.result.value);
    } else if (command === "shot") {
      const result = await cdp.send("Page.captureScreenshot", {
        format: "png",
        captureBeyondViewport: false,
      });
      const fs = await import("node:fs");
      fs.writeFileSync(args[0], Buffer.from(result.data, "base64"));
      console.log(`wrote ${args[0]}`);
    } else if (command === "click") {
      const selector = args[0];
      const result = await cdp.send("Runtime.evaluate", {
        expression: `(()=>{const el=document.querySelector(${JSON.stringify(selector)});if(!el)return 'missing';el.click();return 'clicked';})()`,
        returnByValue: true,
      });
      console.log(result.result.value);
    } else if (command === "type") {
      const [selector, ...rest] = args;
      const text = rest.join(" ");
      const expression = `(()=>{
        const el=document.querySelector(${JSON.stringify(selector)});
        if(!el)return 'missing';
        el.focus();
        const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
        const setter = Object.getOwnPropertyDescriptor(proto,'value').set;
        setter.call(el, ${JSON.stringify(text)});
        el.dispatchEvent(new Event('input',{bubbles:true}));
        el.dispatchEvent(new Event('change',{bubbles:true}));
        return 'typed';
      })()`;
      const result = await cdp.send("Runtime.evaluate", { expression, returnByValue: true });
      console.log(result.result.value);
    } else if (command === "key") {
      const key = args[0] || "Enter";
      const code = args[1] || key;
      const text = args[2] || "";
      const vk = key === "Enter" ? 13 : key === "Escape" ? 27 : 0;
      await cdp.send("Input.dispatchKeyEvent", {
        type: "keyDown",
        key,
        code,
        windowsVirtualKeyCode: vk,
        nativeVirtualKeyCode: vk,
        text,
        unmodifiedText: text,
      });
      if (text) {
        await cdp.send("Input.dispatchKeyEvent", {
          type: "char",
          key,
          code,
          text,
          unmodifiedText: text,
        });
      }
      await cdp.send("Input.dispatchKeyEvent", {
        type: "keyUp",
        key,
        code,
        windowsVirtualKeyCode: vk,
        nativeVirtualKeyCode: vk,
      });
      console.log(`key ${key}`);
    } else if (command === "tab") {
      await cdp.send("Input.dispatchKeyEvent", {
        type: "rawKeyDown",
        key: "Tab",
        code: "Tab",
        windowsVirtualKeyCode: 9,
        nativeVirtualKeyCode: 9,
      });
      await cdp.send("Input.dispatchKeyEvent", {
        type: "keyUp",
        key: "Tab",
        code: "Tab",
        windowsVirtualKeyCode: 9,
        nativeVirtualKeyCode: 9,
      });
      console.log("tab");
    } else if (command === "shiftenter") {
      await cdp.send("Input.dispatchKeyEvent", {
        type: "keyDown",
        key: "Enter",
        code: "Enter",
        modifiers: 8,
        windowsVirtualKeyCode: 13,
        nativeVirtualKeyCode: 13,
        text: "\r",
        unmodifiedText: "\r",
      });
      await cdp.send("Input.dispatchKeyEvent", {
        type: "keyUp",
        key: "Enter",
        code: "Enter",
        modifiers: 8,
        windowsVirtualKeyCode: 13,
        nativeVirtualKeyCode: 13,
      });
      console.log("shift+enter");
    } else if (command === "rawkey") {
      const key = args[0] || "Process";
      const vk = Number(args[1] || 229);
      await cdp.send("Input.dispatchKeyEvent", {
        type: "rawKeyDown",
        key,
        code: "Enter",
        windowsVirtualKeyCode: vk,
        nativeVirtualKeyCode: vk,
      });
      await cdp.send("Input.dispatchKeyEvent", {
        type: "keyUp",
        key,
        code: "Enter",
        windowsVirtualKeyCode: vk,
        nativeVirtualKeyCode: vk,
      });
      console.log(`rawkey ${key} ${vk}`);
    } else if (command === "composition") {
      const text = args[0] || "";
      const selection = Number(args[1] || text.length);
      await cdp.send("Input.imeSetComposition", {
        text,
        selectionStart: selection,
        selectionEnd: selection,
        replacementStart: 0,
        replacementEnd: 0,
      });
      console.log(`composition ${JSON.stringify(text)}`);
    } else if (command === "inserttext") {
      await cdp.send("Input.insertText", { text: args.join(" ") });
      console.log("inserted");
    } else if (command === "wait") {
      await new Promise((r) => setTimeout(r, Number(args[0] || 1000)));
      console.log("waited");
    } else if (command === "files") {
      // Sets files on a real <input type=file> through the DOM domain.
      const [selector, ...rest] = args;
      const path = rest.join(" ");
      const document = await cdp.send("DOM.getDocument", { depth: -1 });
      const found = await cdp.send("DOM.querySelector", {
        nodeId: document.root.nodeId,
        selector,
      });
      if (!found.nodeId) throw new Error(`selector not found: ${selector}`);
      await cdp.send("DOM.setFileInputFiles", { nodeId: found.nodeId, files: [path] });
      console.log(`files ${path}`);
    } else if (command === "dialog") {
      // Fills a native Windows file dialog (Tauri uses rfd, not an HTML input).
      const [path, title] = args;
      const { execFileSync } = await import("node:child_process");
      const script = new URL("./file-dialog.ps1", import.meta.url).pathname.replace(/^\//, "");
      const result = execFileSync(
        "pwsh",
        ["-NoProfile", "-File", script, "-Path", path, "-Title", title || ""],
        { encoding: "utf8" },
      );
      console.log(result.trim());
    } else {
      throw new Error(`unknown command: ${command}`);
    }
  } finally {
    cdp.close();
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
