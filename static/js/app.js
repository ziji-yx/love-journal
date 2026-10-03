"use strict";

const TOKEN_KEY = "love_journal_token";
const state = {
  token: localStorage.getItem(TOKEN_KEY) || "",
};

const MAX_PHOTO_BYTES = 10 * 1024 * 1024;
const MAX_PHOTOS_PER_REQUEST = 8;

const initialMonth = new Date();
state.monthCursor = new Date(initialMonth.getFullYear(), initialMonth.getMonth(), 1);

const view = document.querySelector("#view");
const loginView = document.querySelector("#login");
const appView = document.querySelector("#app");
const loginForm = document.querySelector("#login-form");
const passwordInput = document.querySelector("#password");
const loginError = document.querySelector("#login-err");

function esc(value) {
  return String(value ?? "").replace(/[&<>"']/g, (char) => {
    const entities = {
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&#039;",
    };
    return entities[char];
  });
}

function authorLabel(author) {
  if (author === "me") return "我";
  if (author === "her") return "她";
  return esc(author) || "我们";
}

function formatDate(date) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date || "");
  if (!match) return esc(date);
  return `${Number(match[1])}年${Number(match[2])}月${Number(match[3])}日`;
}

function formatMonth(yearMonth) {
  const match = /^(\d{4})-(\d{2})$/.exec(yearMonth || "");
  if (!match) return esc(yearMonth);
  return `${Number(match[1])}年${Number(match[2])}月`;
}

function formatDateTime(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return esc(value);
  return new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function photoUrl(photo) {
  return `/uploads/${encodeURIComponent(photo.filename)}`;
}

function photoThumb(entry) {
  if (entry.photos && entry.photos.length > 0) {
    return `<img src="${photoUrl(entry.photos[0])}" alt="手账照片" loading="lazy" />`;
  }
  return "♥";
}

function authorBadge(author) {
  return `<span class="author">${authorLabel(author)}</span>`;
}
function commentTemplate(comment) {
  return `
    <div class="comment">
      <div class="sticker">${esc(comment.sticker || "💬")}</div>
      <div>
        <span class="who">${authorLabel(comment.author)}</span>
        <span class="when">${formatDateTime(comment.created_at)}</span>
        <div>${esc(comment.content)}</div>
      </div>
    </div>
  `;
}

function renderEntryCard(entry, compact = false) {
  const note = esc(entry.note || "");
  return `
    <article class="entry${compact ? " compact" : ""}" data-entry-id="${entry.id}">
      <div class="thumb">${photoThumb(entry)}</div>
      <div>
        <div class="meta">${formatDate(entry.date)}</div>
        <div>${authorBadge(entry.author)}</div>
        <p class="note">${note || '<span class="muted-text">没有写下内容</span>'}</p>
      </div>
    </article>
  `;
}

function emptyState(message) {
  return `<div class="empty">${esc(message)}</div>`;
}

function setAuth(token) {
  state.token = token || "";
  if (token) {
    localStorage.setItem(TOKEN_KEY, token);
  } else {
    localStorage.removeItem(TOKEN_KEY);
  }
}

async function api(path, options = {}) {
  const headers = new Headers(options.headers || {});
  if (state.token) {
    headers.set("Authorization", `Bearer ${state.token}`);
  }
  if (options.body && !(options.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }

  const response = await fetch(path, { credentials: "same-origin", ...options, headers, cache: "no-store" });
  const raw = await response.text();
  let data = {};
  try {
    data = raw ? JSON.parse(raw) : {};
  } catch {
    data = { error: "服务器返回了无法识别的数据" };
  }

  if (response.status === 401) {
    setAuth("");
    showLogin("登录已过期，请重新进入");
    throw new Error(data.error || "登录已过期");
  }

  if (!response.ok) {
    throw new Error(data.error || `请求失败（${response.status}）`);
  }

  return data;
}

function showLogin(message = "") {
  loginView.hidden = false;
  appView.hidden = true;
  loginError.textContent = message;
  if (!message) {
    setTimeout(() => passwordInput.focus(), 0);
  }
}

function showApp() {
  loginView.hidden = true;
  appView.hidden = false;
  render();
}

function toast(message, type = "error") {
  let el = document.querySelector("#toast");
  if (!el) {
    el = document.createElement("div");
    el.id = "toast";
    el.setAttribute("role", "status");
    document.body.append(el);
  }
  el.textContent = message;
  el.className = `toast show ${type}`;
  window.clearTimeout(toast.timer);
  toast.timer = window.setTimeout(() => el.classList.remove("show"), 2800);
}

async function handleLogin(event) {
  event.preventDefault();
  const password = passwordInput.value;
  loginError.textContent = "";
  const button = loginForm.querySelector('button[type="submit"]');
  button.disabled = true;
  button.textContent = "正在进入…";

  try {
    const data = await api("/api/login", {
      method: "POST",
      body: JSON.stringify({ password }),
    });
    setAuth(data.token);
    passwordInput.value = "";
    showApp();
  } catch (error) {
    loginError.textContent = error.message || "进入失败";
  } finally {
    button.disabled = false;
    button.textContent = "进入";
  }
}

async function handleLogout() {
  try {
    await api("/api/logout", { method: "POST" });
  } catch {
    // Local session removal is enough even if the network call fails.
  }
  setAuth("");
  showLogin();
}

function navigate(hash) {
  if (location.hash === hash) {
    render();
  } else {
    location.hash = hash;
  }
}

function currentRoute() {
  const hash = location.hash || "#/";
  return hash.replace(/^#/, "");
}

function render() {
  if (!state.token) {
    showLogin();
    return;
  }

  appView.hidden = false;
  loginView.hidden = true;

  const route = currentRoute();
  const updateLinks = () => {
    document.querySelectorAll(".topbar nav a").forEach((link) => {
      const target = `#${link.getAttribute("href").slice(1)}`;
      link.setAttribute("aria-current", route === target ? "page" : "false");
    });
  };

  if (route === "/") {
    renderHome().finally(updateLinks);
  } else if (route === "/timeline") {
    renderTimeline().finally(updateLinks);
  } else if (route === "/calendar") {
    renderCalendar().finally(updateLinks);
  } else if (route === "/new") {
    renderEntryForm();
    updateLinks();
  } else if (/^\/edit\/\d+$/.test(route)) {
    const id = Number(route.split("/")[2]);
    renderEntryForm(id).finally(updateLinks);
  } else if (/^\/entry\/\d+$/.test(route)) {
    const id = Number(route.split("/")[2]);
    renderEntryDetail(id).finally(updateLinks);
  } else {
    view.innerHTML = emptyState("这个页面不存在");
    view.insertAdjacentHTML(
      "beforeend",
      '<div class="form-actions"><a class="btn" href="#/">回到首页</a></div>',
    );
    updateLinks();
  }
}

async function renderHome() {
  view.innerHTML = '<div class="loading">正在整理我们的回忆…</div>';
  try {
    const [stats, entries] = await Promise.all([
      api("/api/stats"),
      api("/api/entries"),
    ]);

    const nextAnniversary = stats.next_anniversary
      ? `
        <p class="anniv">
          下一个纪念日：<b>${esc(stats.next_anniversary.name)}</b><br />
          ${formatDate(stats.next_anniversary.date)}，还有
          <b>${stats.next_anniversary.days_until}</b> 天
        </p>`
      : '<p class="anniv">往后每一天，都值得纪念</p>';

    const latest = entries.slice(0, 5);
    const recentHtml = latest.length
      ? latest.map((entry) => renderEntryCard(entry)).join("")
      : emptyState("还没有手账，从第一篇开始吧");

    view.innerHTML = `
      <section class="hero">
        <article class="card">
          <div class="label">我们在一起</div>
          <div class="days">${Number(stats.days_together)}</div>
          <div class="label">天</div>
          <p class="anniv">从 ${formatDate(stats.love_start)} 开始</p>
        </article>
        <article class="card">
          <div class="label">下一个纪念日</div>
          ${nextAnniversary}
        </article>
      </section>

      <section>
        <div class="section-title">
          <h2>最近手账</h2>
          <button id="random-entry" class="btn sm" type="button">随机回忆</button>
        </div>
        <div class="entry-list">${recentHtml}</div>
      </section>
    `;

    document.querySelector("#random-entry").addEventListener("click", handleRandomEntry);
  } catch (error) {
    view.innerHTML = emptyState(error.message || "加载失败");
  }
}

async function handleRandomEntry() {
  const button = document.querySelector("#random-entry");
  if (!button) return;
  button.disabled = true;
  button.textContent = "正在寻找…";
  try {
    const entry = await api("/api/random");
    if (entry) {
      navigate(`#/entry/${entry.id}`);
    } else {
      toast("还没有可以随机回忆的手账");
    }
  } catch (error) {
    toast(error.message || "随机回忆失败");
  } finally {
    button.disabled = false;
    button.textContent = "随机回忆";
  }
}

async function renderTimeline() {
  view.innerHTML = '<div class="loading">正在展开时间轴…</div>';
  try {
    const entries = await api("/api/entries");
    if (!entries.length) {
      view.innerHTML = emptyState("时间轴还是空的，写下第一篇手账吧");
      return;
    }

    const groups = new Map();
    entries.forEach((entry) => {
      const month = String(entry.date).slice(0, 7);
      if (!groups.has(month)) groups.set(month, []);
      groups.get(month).push(entry);
    });

    view.innerHTML = [...groups.entries()]
      .map(
        ([month, monthEntries]) => `
          <section class="timeline-group">
            <h3>${formatMonth(month)}</h3>
            <div class="entry-list">${monthEntries
              .map((entry) => renderEntryCard(entry))
              .join("")}</div>
          </section>
        `,
      )
      .join("");
  } catch (error) {
    view.innerHTML = emptyState(error.message || "时间轴加载失败");
  }
}

function calendarMonthLabel(date) {
  return `${date.getFullYear()}年${date.getMonth() + 1}月`;
}

function dateKey(year, month, day) {
  return `${year}-${String(month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

async function renderCalendar() {
  view.innerHTML = '<div class="loading">正在翻开日历…</div>';
  try {
    const entries = await api("/api/entries");
    const byDate = new Map();
    entries.forEach((entry) => byDate.set(entry.date, entry));

    const cursor = state.monthCursor;
    const year = cursor.getFullYear();
    const month = cursor.getMonth();
    const daysInMonth = new Date(year, month + 1, 0).getDate();
    const firstOffset = (new Date(year, month, 1).getDay() + 6) % 7;
    const today = new Date();
    const todayKey = dateKey(
      today.getFullYear(),
      today.getMonth(),
      today.getDate(),
    );

    let days = "";
    for (let i = 0; i < firstOffset; i += 1) {
      days += '<div class="day empty"></div>';
    }
    for (let day = 1; day <= daysInMonth; day += 1) {
      const key = dateKey(year, month, day);
      const entry = byDate.get(key);
      const has = Boolean(entry);
      const isToday = key === todayKey;
      days += `
        <div
          class="day${has ? " has" : ""}${isToday ? " today" : ""}"
          data-date="${key}"
          ${has ? `data-entry-id="${entry.id}"` : ""}
        >${day}</div>
      `;
    }

    view.innerHTML = `
      <section class="card calendar-card">
        <div class="calendar-head">
          <button id="calendar-prev" class="btn ghost sm" type="button" aria-label="上个月">‹</button>
          <h2>${calendarMonthLabel(cursor)}</h2>
          <button id="calendar-next" class="btn ghost sm" type="button" aria-label="下个月">›</button>
        </div>
        <div class="calendar">
          <div class="dow">一</div>
          <div class="dow">二</div>
          <div class="dow">三</div>
          <div class="dow">四</div>
          <div class="dow">五</div>
          <div class="dow">六</div>
          <div class="dow">日</div>
          ${days}
        </div>
      </section>
    `;

    document.querySelector("#calendar-prev").addEventListener("click", () => {
      state.monthCursor = new Date(
        state.monthCursor.getFullYear(),
        state.monthCursor.getMonth() - 1,
        1,
      );
      renderCalendar();
    });
    document.querySelector("#calendar-next").addEventListener("click", () => {
      state.monthCursor = new Date(
        state.monthCursor.getFullYear(),
        state.monthCursor.getMonth() + 1,
        1,
      );
      renderCalendar();
    });
  } catch (error) {
    view.innerHTML = emptyState(error.message || "日历加载失败");
  }
}

function todayInputValue() {
  const today = new Date();
  return dateKey(today.getFullYear(), today.getMonth(), today.getDate());
}

async function renderEntryForm(id = null) {
  const isEdit = id !== null;
  let entry = null;

  if (isEdit) {
    view.innerHTML = '<div class="loading">正在取出这一页…</div>';
    try {
      entry = await api(`/api/entries/${id}`);
    } catch (error) {
      view.innerHTML = emptyState(error.message || "手账加载失败");
      return;
    }
  }

  view.innerHTML = `
    <section class="card">
      <div class="detail-head">
        <h2>${isEdit ? "修改手账" : "写下新的手账"}</h2>
        <a class="btn ghost sm" href="#/${isEdit ? `entry/${id}` : ""}">返回</a>
      </div>
      <form id="entry-form" class="form-grid">
        ${isEdit ? `<input id="entry-version" type="hidden" value="${entry.version}" />` : ""}
        <label>
          日期
          <input id="entry-date" type="date" value="${entry?.date || todayInputValue()}" required />
        </label>
        <label>
          是谁写的
          <select id="entry-author">
            <option value="me"${entry?.author === "her" ? "" : " selected"}>我</option>
            <option value="her"${entry?.author === "her" ? " selected" : ""}>她</option>
          </select>
        </label>
        <label>
          这一天发生了什么
          <textarea id="entry-note" maxlength="10000" placeholder="把想记住的小事写下来…" required>${esc(
            entry?.note || "",
          )}</textarea>
        </label>
        <div class="form-actions">
          <button class="btn primary" type="submit">${isEdit ? "保存修改" : "收进手账"}</button>
        </div>
      </form>
    </section>
  `;

  document.querySelector("#entry-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const date = document.querySelector("#entry-date").value;
    const author = document.querySelector("#entry-author").value;
    const note = document.querySelector("#entry-note").value.trim();
    if (!date || !note) {
      toast("日期和内容都需要填写");
      return;
    }

    const button = event.currentTarget.querySelector('button[type="submit"]');
    button.disabled = true;
    button.textContent = isEdit ? "正在保存…" : "正在收藏…";
    try {
      const saved = await api(isEdit ? `/api/entries/${id}` : "/api/entries", {
        method: isEdit ? "PUT" : "POST",
        body: JSON.stringify(
          isEdit
            ? {
                date,
                author,
                note,
                version: entry.version,
              }
            : { date, author, note },
        ),
      });
      toast(isEdit ? "修改已保存" : "已经收进手账", "success");
      navigate(`#/entry/${saved.id}`);
    } catch (error) {
      toast(error.message || "保存失败");
      if (/刚刚修改过/.test(error.message || "")) {
        navigate(`#/entry/${id}`);
      }
    } finally {
      button.disabled = false;
      button.textContent = isEdit ? "保存修改" : "收进手账";
    }
  });
}

async function renderEntryDetail(id) {
  view.innerHTML = '<div class="loading">正在翻开这一页…</div>';
  let detail;
  try {
    detail = await api(`/api/entries/${id}`);
  } catch (error) {
    view.innerHTML = emptyState(error.message || "手账加载失败");
    return;
  }

  const entry = detail;
  const photos = detail.photos || [];
  const comments = detail.comments || [];

  const photosHtml = photos.length
    ? photos
        .map(
          (photo) => `
            <div class="photo-wrap">
              <img
                src="${photoUrl(photo)}"
                alt="${esc(photo.original_name)}"
                loading="lazy"
                data-photo-url="${photoUrl(photo)}"
              />
              <button class="photo-del" type="button" title="删除照片" data-photo-delete="${photo.id}">×</button>
            </div>
          `,
        )
        .join("")
    : '<div class="empty">还没有照片，上传一张留住这一刻吧</div>';

  const commentsHtml = comments.length
    ? comments
        .map(
          (comment) => commentTemplate(comment),
        )
        .join("")
    : '<div class="empty">还没有留言，说一句悄悄话吧</div>';

  view.innerHTML = `
    <article class="card">
      <div class="detail-head">
        <div>
          <div class="detail-date">${formatDate(entry.date)}</div>
          <div class="meta">${authorBadge(entry.author)} · 更新于 ${formatDateTime(
            entry.updated_at,
          )}</div>
        </div>
        <div class="detail-actions">
          <a class="btn ghost sm" href="#/edit/${entry.id}">修改</a>
          <button id="delete-entry" class="btn danger sm" type="button">删除</button>
        </div>
      </div>
      <div class="detail-note">${esc(entry.note)}</div>
    </article>

    <section class="card">
      <div class="section-title">
        <h2>照片</h2>
        <label class="upload-button">
          上传照片
          <input id="photo-input" type="file" accept="image/jpeg,image/png,image/gif,image/webp" multiple hidden />
        </label>
      </div>
      <div class="photos">${photosHtml}</div>
    </section>

    <section class="card">
      <div class="section-title">
        <h2>留言</h2>
      </div>
      <div id="comment-list">${commentsHtml}</div>
      <form id="comment-form" class="comment-form">
        <input id="comment-author" type="text" maxlength="20" placeholder="我 / 她 / 昵称" required />
        <input id="comment-content" type="text" maxlength="500" placeholder="想说点什么…" required />
        <div class="sticker-pick" aria-label="选择贴纸">
          <button class="selected" type="button" data-sticker="💗">💗</button>
          <button type="button" data-sticker="🌙">🌙</button>
          <button type="button" data-sticker="✨">✨</button>
          <button type="button" data-sticker="🍰">🍰</button>
          <button type="button" data-sticker="🥰">🥰</button>
        </div>
        <button id="comment-submit" class="btn primary sm" type="submit">留言</button>
      </form>
    </section>
  `;

  document.querySelector("#delete-entry").addEventListener("click", () => handleDeleteEntry(id));
  document.querySelector("#photo-input").addEventListener("change", (event) => handlePhotoUpload(id, event));
  document.querySelector("#comment-form").addEventListener("submit", (event) => handleCommentSubmit(id, event));
  document.querySelectorAll("[data-sticker]").forEach((button) => {
    button.addEventListener("click", () => {
      document.querySelector("#comment-author").value = "me";
      document.querySelectorAll("[data-sticker]").forEach((item) => item.classList.remove("selected"));
      button.classList.add("selected");
    });
  });
  document.querySelectorAll("[data-photo-delete]").forEach((button) => {
    button.addEventListener("click", () => handlePhotoDelete(button.dataset.photoDelete));
  });
  document.querySelectorAll("[data-photo-url]").forEach((image) => {
    image.addEventListener("click", () => openLightbox(image.dataset.photoUrl));
  });
}

async function handleDeleteEntry(id) {
  if (!window.confirm("确定删除这篇手账吗？照片和留言也会一起删除。")) return;
  const button = document.querySelector("#delete-entry");
  if (button) {
    button.disabled = true;
    button.textContent = "正在删除…";
  }
  try {
    await api(`/api/entries/${id}`, { method: "DELETE" });
    toast("这篇手账已删除", "success");
    navigate("#/");
  } catch (error) {
    toast(error.message || "删除失败");
    if (button) {
      button.disabled = false;
      button.textContent = "删除";
    }
  }
}

async function handlePhotoUpload(id, event) {
  const files = [...event.target.files];
  if (!files.length) return;

  if (files.length > MAX_PHOTOS_PER_REQUEST) {
    toast(`一次最多上传 ${MAX_PHOTOS_PER_REQUEST} 张照片`);
    event.target.value = "";
    return;
  }
  const oversized = files.find((file) => file.size > MAX_PHOTO_BYTES);
  if (oversized) {
    toast(`“${oversized.name}”超过 10 MB`);
    event.target.value = "";
    return;
  }


  const button = event.target;
  button.disabled = true;
  try {
    const formData = new FormData();
    files.forEach((file) => formData.append("photos", file));
    await api(`/api/entries/${id}/photos`, {
      method: "POST",
      body: formData,
    });
    toast(`已上传 ${files.length} 张照片`, "success");
    await renderEntryDetail(id);
  } catch (error) {
    toast(error.message || "照片上传失败");
  } finally {
    if (!button.isConnected) {
      const currentInput = document.querySelector("#photo-input");
      if (currentInput) currentInput.disabled = false;
    } else {
      button.disabled = false;
    }
  }
}

async function handlePhotoDelete(photoId) {
  if (!window.confirm("确定删除这张照片吗？")) return;
  try {
    await api(`/api/photos/${encodeURIComponent(photoId)}`, { method: "DELETE" });
    toast("照片已删除", "success");
    const match = /^\/entry\/(\d+)$/.exec(currentRoute());
    if (match) await renderEntryDetail(Number(match[1]));
  } catch (error) {
    toast(error.message || "删除失败");
  }
}

async function handleCommentSubmit(id, event) {
  event.preventDefault();
  const authorInput = document.querySelector("#comment-author");
  const contentInput = document.querySelector("#comment-content");
  const selected = document.querySelector("[data-sticker].selected");
  const sticker = selected ? selected.dataset.sticker : "💗";
  const author = authorInput.value.trim();
  const content = contentInput.value.trim();
  if (!author || !content) {
    toast("留下名字和想说的话");
    return;
  }

  const button = event.currentTarget.querySelector('button[type="submit"]');
  button.disabled = true;
  button.textContent = "正在留言…";
  try {
    const created = await api(`/api/entries/${id}/comments`, {
      method: "POST",
      body: JSON.stringify({ author, content, sticker }),
    });
    authorInput.value = "";
    contentInput.value = "";
    toast("留言已收好", "success");
    const list = document.querySelector("#comment-list");
    const empty = list.querySelector(".empty");
    if (empty) empty.remove();
    list.insertAdjacentHTML("beforeend", commentTemplate(created));
  } catch (error) {
    toast(error.message || "留言失败");
  } finally {
    button.disabled = false;
    button.textContent = "留言";
  }
}

function openLightbox(url) {
  const lightbox = document.querySelector("#lightbox");
  const image = document.querySelector("#lightbox-img");
  image.src = url;
  image.alt = "放大的照片";
  lightbox.hidden = false;
  document.body.classList.add("no-scroll");
}

function closeLightbox() {
  const lightbox = document.querySelector("#lightbox");
  lightbox.hidden = true;
  document.body.classList.remove("no-scroll");
}

view.addEventListener("click", (event) => {
  const photo = event.target.closest("[data-photo-url]");
  if (photo) {
    openLightbox(photo.dataset.photoUrl);
    return;
  }

  const entryCard = event.target.closest("[data-entry-id]");
  if (entryCard) {
    const interactive = event.target.closest("a, button, input, select, textarea");
    if (!interactive) {
      navigate(`#/entry/${entryCard.dataset.entryId}`);
    }
  }
});

document.querySelector("#lightbox").addEventListener("click", closeLightbox);
document.querySelector("#logout").addEventListener("click", handleLogout);
loginForm.addEventListener("submit", handleLogin);
window.addEventListener("hashchange", render);
window.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeLightbox();
});

async function boot() {
  if (!state.token) {
    showLogin();
    return;
  }
  try {
    await api("/api/me");
    showApp();
  } catch {
    showLogin();
  }
}

boot();
