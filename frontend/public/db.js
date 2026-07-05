export function openDB(name, version, upgradeFn) {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(name, version);
    req.onupgradeneeded = (e) => upgradeFn(e.target.result);
    req.onsuccess = (e) => resolve(e.target.result);
    req.onerror = (e) => reject(req.error);
  });
}

export function saveTask(db, task) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction("tasks", "readwrite");
    const store = tx.objectStore("tasks");
    const req = store.put(task);
    req.onsuccess = () => resolve();
    req.onerror = () => reject(req.error);
  });
}

export function saveTasks(db, tasks) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction("tasks", "readwrite");
    const store = tx.objectStore("tasks");
    let completed = 0;
    for (const task of tasks) {
      const req = store.put(task);
      req.onsuccess = () => {
        completed++;
        if (completed === tasks.length) resolve();
      };
      req.onerror = () => reject(req.error);
    }
    if (tasks.length === 0) resolve();
  });
}

export function listTasks(db) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction("tasks", "readonly");
    const store = tx.objectStore("tasks");
    const req = store.getAll();
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export function deleteTask(db, id) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction("tasks", "readwrite");
    const store = tx.objectStore("tasks");
    const req = store.delete(id);
    req.onsuccess = () => resolve();
    req.onerror = () => reject(req.error);
  });
}

export function clearTasks(db) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction("tasks", "readwrite");
    const store = tx.objectStore("tasks");
    const req = store.clear();
    req.onsuccess = () => resolve();
    req.onerror = () => reject(req.error);
  });
}