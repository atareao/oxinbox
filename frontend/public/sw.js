const CACHE_NAME = "oxinbox-v2";
const DYNAMIC_CACHE = "oxinbox-dynamic-v1";

const PRECACHE_URLS = ["/", "/styles.css"];

self.addEventListener("install", (event) => {
  self.skipWaiting();
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(PRECACHE_URLS)),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter((k) => k !== CACHE_NAME && k !== DYNAMIC_CACHE)
          .map((k) => caches.delete(k)),
      ),
    ),
  );
});

self.addEventListener("fetch", (event) => {
  const { request } = event;
  const url = new URL(request.url);

  if (url.pathname.startsWith("/api/")) {
    event.respondWith(networkFirst(request));
  } else {
    event.respondWith(cacheFirst(request));
  }
});

async function networkFirst(request) {
  try {
    const response = await fetch(request);
    const cache = await caches.open(DYNAMIC_CACHE);
    cache.put(request, response.clone());
    return response;
  } catch {
    const cached = await caches.match(request);
    if (cached) return cached;
    return new Response(JSON.stringify({ offline: true }), {
      headers: { "Content-Type": "application/json" },
    });
  }
}

async function cacheFirst(request) {
  const cached = await caches.match(request);
  if (cached) return cached;
  try {
    return await fetch(request);
  } catch {
    return new Response("Offline", { status: 503 });
  }
}

self.addEventListener("push", (event) => {
  let data = { title: "oxinbox", body: "Tienes una actualización" };
  if (event.data) {
    try {
      data = event.data.json();
    } catch {
      data.body = event.data.text();
    }
  }
  event.waitUntil(
    self.registration.showNotification(data.title, {
      body: data.body,
      icon: "/icon-192.png",
      badge: "/icon-192.png",
      vibrate: [200, 100, 200],
    }),
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil(clients.openWindow("/"));
});

self.addEventListener("sync", (event) => {
  if (event.tag === "sync-tasks") {
    event.waitUntil(syncPendingOps());
  }
});

async function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open("oxinbox", 2);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function syncPendingOps() {
  let db;
  try {
    db = await openDb();
  } catch {
    return;
  }

  const tx = db.transaction("pending_ops", "readonly");
  const store = tx.objectStore("pending_ops");
  const allOps = await new Promise((resolve, reject) => {
    const req = store.getAll();
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });

  if (!allOps || allOps.length === 0) return;

  const sorted = allOps.sort((a, b) => a.created_at - b.created_at);

  for (const op of sorted) {
    try {
      const headers = {
        "Content-Type": "application/json",
        Authorization: `Bearer ${op.token}`,
      };
      const fetchOpts = {
        method: op.method,
        headers,
      };
      if (op.body && (op.method === "POST" || op.method === "PUT")) {
        fetchOpts.body = JSON.stringify(op.body);
      }
      const resp = await fetch(op.url, fetchOpts);
      if (resp.ok) {
        const deleteTx = db.transaction("pending_ops", "readwrite");
        const deleteStore = deleteTx.objectStore("pending_ops");
        await new Promise((resolve, reject) => {
          const req = deleteStore.delete(op.id);
          req.onsuccess = () => resolve();
          req.onerror = () => reject(req.error);
        });
      }
    } catch {
      // will retry on next sync event
    }
  }
  db.close();
}