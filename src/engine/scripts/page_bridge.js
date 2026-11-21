(() => {
  const post = globalThis.__wazzupPost;
  if (typeof post !== "function") {
    return;
  }
  delete globalThis.__wazzupPost;

  const hidden = (value) => ({ value, writable: true, configurable: true, enumerable: false });
  const send = (message) => post(JSON.stringify(message));

  const sharedRegistry = () => {
    let owner = globalThis;
    try {
      if (globalThis.top && globalThis.top.location.origin === globalThis.location.origin) {
        owner = globalThis.top;
      }
    } catch (_) {
      owner = globalThis;
    }
    if (!owner.__wazzupRegistry) {
      Object.defineProperty(owner, "__wazzupRegistry", hidden({ live: new Map(), sequence: 0 }));
      Object.defineProperty(owner, "__wazzupActivate", hidden((id) => owner.__wazzupRegistry.live.get(id)?.()));
    }
    return owner.__wazzupRegistry;
  };
  const registry = sharedRegistry();

  class DesktopNotification extends EventTarget {
    #id;

    constructor(title, options = {}) {
      super();
      this.#id = ++registry.sequence;
      this.title = String(title ?? "");
      this.body = String(options.body ?? "");
      this.tag = String(options.tag ?? "");
      this.icon = String(options.icon ?? "");
      this.data = options.data ?? null;
      this.silent = Boolean(options.silent);
      this.onclick = null;
      this.onclose = null;
      this.onshow = null;
      this.onerror = null;
      registry.live.set(this.#id, () => this.#raise("click"));
      send({ kind: "notification", id: this.#id, title: this.title, body: this.body });
      queueMicrotask(() => this.#raise("show"));
    }

    static get permission() {
      return "granted";
    }

    static get maxActions() {
      return 0;
    }

    static requestPermission(callback) {
      if (typeof callback === "function") {
        callback("granted");
      }
      return Promise.resolve("granted");
    }

    close() {
      if (registry.live.delete(this.#id)) {
        send({ kind: "notification-closed", id: this.#id });
        this.#raise("close");
      }
    }

    #raise(type) {
      const event = new Event(type);
      this.dispatchEvent(event);
      const handler = this[`on${type}`];
      if (typeof handler === "function") {
        handler.call(this, event);
      }
    }
  }

  const installNotification = () => {
    if (globalThis.Notification !== DesktopNotification) {
      Object.defineProperty(globalThis, "Notification", hidden(DesktopNotification));
    }
  };
  installNotification();
  queueMicrotask(installNotification);
  setTimeout(installNotification, 0);
  document.addEventListener("readystatechange", installNotification);

  if (registry === globalThis.__wazzupRegistry) {
    const veilSelectors = [
      "#pane-side [role='row']",
      "#pane-side [role='listitem']",
      "#main [role='row']",
      "#main [role='application'] img",
      "#main video",
    ];
    const veilCss = (pixels) =>
      pixels > 0
        ? `${veilSelectors.join(",")}{filter:blur(${pixels}px);transition:filter .15s ease}` +
          `${veilSelectors.map((selector) => `${selector}:hover`).join(",")}{filter:none}`
        : "";
    const applyVeil = (pixels) => {
      let style = document.getElementById("wazzup-veil");
      if (!style) {
        style = document.createElement("style");
        style.id = "wazzup-veil";
        (document.head ?? document.documentElement).append(style);
      }
      style.textContent = veilCss(Number(pixels) || 0);
    };
    Object.defineProperty(globalThis, "__wazzupVeil", hidden(applyVeil));

    const NativeSocket = globalThis.WebSocket;
    const watchedHost = /(^|\.)whatsapp\.(net|com)$/i;
    let openSockets = 0;
    const report = (online) => send({ kind: "connection", online });
    class WatchedSocket extends NativeSocket {
      constructor(address, protocols) {
        super(address, protocols);
        let host = "";
        try {
          host = new URL(String(address), globalThis.location.href).hostname;
        } catch (_) {
          host = "";
        }
        if (!watchedHost.test(host)) {
          return;
        }
        let counted = false;
        this.addEventListener("open", () => {
          counted = true;
          openSockets += 1;
          if (openSockets === 1) {
            report(true);
          }
        });
        this.addEventListener("close", () => {
          if (!counted) {
            return;
          }
          counted = false;
          openSockets = Math.max(0, openSockets - 1);
          if (openSockets === 0) {
            report(false);
          }
        });
      }
    }
    Object.defineProperty(WatchedSocket, "name", { value: "WebSocket" });
    Object.defineProperty(globalThis, "WebSocket", hidden(WatchedSocket));
  }

  const alwaysPersistent = (target) => {
    if (target) {
      target.persist = () => Promise.resolve(true);
      target.persisted = () => Promise.resolve(true);
    }
    return target;
  };
  alwaysPersistent(navigator.storage);

  const buckets = navigator.storageBuckets;
  if (buckets?.open) {
    const open = buckets.open.bind(buckets);
    buckets.open = (name, options = {}) =>
      open(name, { ...options, persisted: false }).then(alwaysPersistent);
  }

  const permissions = navigator.permissions;
  if (permissions?.query) {
    const query = permissions.query.bind(permissions);
    const grantedStatus = (name) =>
      Object.assign(new EventTarget(), { name, state: "granted", onchange: null });
    const alwaysGranted = new Set(["notifications", "persistent-storage", "microphone", "camera"]);
    permissions.query = (descriptor) =>
      alwaysGranted.has(descriptor?.name)
        ? Promise.resolve(grantedStatus(descriptor.name))
        : query(descriptor);
  }
})();
