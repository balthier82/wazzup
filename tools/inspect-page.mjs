const port = process.env.WAZZUP_INSPECTOR_PORT ?? "9333";
const expression = process.argv[2] ?? "document.body.innerText";
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = targets.find((target) => target.type === "page" && target.url.includes("whatsapp"));
if (!page) {
  console.error("no WhatsApp page found", targets.map((target) => target.url));
  process.exit(1);
}
const socket = new WebSocket(page.webSocketDebuggerUrl);
const pending = new Map();
let sequence = 0;
const call = (method, params = {}) =>
  new Promise((resolve) => {
    const id = ++sequence;
    pending.set(id, resolve);
    socket.send(JSON.stringify({ id, method, params }));
  });
socket.onmessage = (event) => {
  const message = JSON.parse(event.data);
  if (message.id && pending.has(message.id)) {
    pending.get(message.id)(message.result);
    pending.delete(message.id);
  }
};
socket.onopen = async () => {
  const outcome = await call("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  console.log(JSON.stringify(outcome?.result?.value ?? outcome, null, 2));
  socket.close();
};
