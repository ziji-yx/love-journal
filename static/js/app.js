"use strict";

const TOKEN_KEY = "love_journal_token";
const THEME_KEY = "love_journal_theme";
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
const searchForm = document.querySelector("#search-form");
const searchInput = document.querySelector("#search-input");
const themeToggle = document.querySelector("#theme-toggle");

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
  const kind = author === "her" ? "her" : "me";
  return `<span class="author author-${kind}">${authorLabel(author)}</span>`;
}
function commentTemplate(comment) {
  return `
    <div class="comment" data-comment-id="${comment.id}">
      <div class="sticker">${esc(comment.sticker || "💬")}</div>
      <div class="comment-body">
        <div class="comment-head">
          <span class="who">${authorLabel(comment.author)}</span>
          <span class="when">${formatDateTime(comment.created_at)}</span>
        </div>
        <div class="comment-text">${esc(comment.content)}</div>
      </div>
      <button class="comment-del" type="button" data-comment-delete="${comment.id}" title="删除留言">×</button>
    </div>
  `;
}

function renderEntryCard(entry, compact = false) {
  const note = esc(entry.note || "");
  return `
    <article class="entry entry-${entry.author === "her" ? "her" : "me"}${compact ? " compact" : ""}" data-entry-id="${entry.id}">
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
function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
  localStorage.setItem(THEME_KEY, theme);
  if (themeToggle) themeToggle.textContent = theme === "dark" ? "☀️" : "🌙";
}
function toggleTheme() {
  const next = document.documentElement.dataset.theme === "dark" ? "light" : "dark";
  applyTheme(next);
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
  } else if (route === "/milestones") {
    window.renderMilestonesPage().finally(updateLinks);
  } else if (/^\/search(?:\/.*)?$/.test(route)) {
    const query = decodeURIComponent(route.slice("/search/".length) || "");
    searchInput.value = query;
    renderSearch(query).finally(updateLinks);
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
function nextMilestoneDate(date, repeatYearly) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date || "");
  if (!match) return null;
  const month = Number(match[2]);
  const day = Number(match[3]);
  const today = new Date();
  const currentYear = today.getFullYear();
  let candidate = new Date(currentYear, month - 1, day);
  if (candidate < new Date(today.getFullYear(), today.getMonth(), today.getDate())) {
    candidate = new Date(currentYear + 1, month - 1, day);
  }
  if (!repeatYearly) {
    const original = new Date(`${date}T00:00:00`);
    candidate = original;
  }
  return candidate;
}
function milestoneCountdown(milestones) {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return milestones
    .map((milestone) => {
      const next = nextMilestoneDate(milestone.date, milestone.repeat_yearly);
      return { ...milestone, nextDate: next };
    })
    .filter((item) => item.nextDate)
    .sort((a, b) => a.nextDate - b.nextDate);
}
function milestoneCard(milestone) {
  const next = milestone.nextDate || new Date(`${milestone.date}T00:00:00`);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const days = Math.max(0, Math.ceil((next - today) / 86400000));
  const emoji = esc(milestone.emoji || "📅");
  return `
    <article class="milestone-card" data-milestone-id="${milestone.id}">
      <div class="milestone-emoji">${emoji}</div>
      <button class="milestone-del" type="button" data-milestone-delete="${milestone.id}" title="删除纪念日">×</button>
      <div class="milestone-main">
        <strong>${esc(milestone.name)}</strong>
        <span>${formatDate(next.toISOString().slice(0, 10))}</span>
      </div>
      <div class="milestone-days">${days}<small>天</small></div>
    </article>
  `;
}
  view.innerHTML = '<div class="loading">正在整理我们的回忆…</div>';
  try {
    const [stats, entries, milestones] = await Promise.all([
      api("/api/stats"),
      api("/api/entries"),
      api("/api/milestones"),
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
    const upcomingMilestones = milestoneCountdown(milestones).slice(0, 3);
    const milestoneHtml = upcomingMilestones.length
      ? upcomingMilestones.map(milestoneCard).join("")
      : '<a class="milestone-empty" href="#/milestones">还没有自定义纪念日，去添加第一个吧</a>';
    const recentHtml = latest.length
      ? latest.map((entry) => renderEntryCard(entry)).join("")
      : emptyState("还没有手账，从第一篇开始吧");
    const now = new Date();
    const greeting = now.getHours() < 11 ? "早上好" : now.getHours() < 18 ? "下午好" : "晚上好";
    const dateLabel = new Intl.DateTimeFormat("zh-CN", {
      year: "numeric",
      month: "long",
      day: "numeric",
      weekday: "long",
    }).format(now);

    view.innerHTML = `
      <header class="dashboard-head">
        <div>
          <span class="eyebrow">OUR LITTLE JOURNAL</span>
          <h1>${greeting}，今天也值得被记住</h1>
          <div class="dashboard-date">${dateLabel}</div>
        </div>
        <div class="dashboard-actions">
          <button id="random-entry-top" class="btn ghost sm" type="button">随机回忆</button>
          <a class="btn primary sm" href="#/new">写下今天</a>
        </div>
      </header>
      <div class="home-grid">
        <div class="home-main">
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
          <div class="stat-strip">
            <div class="stat-item stat-entries"><span class="stat-icon">📖</span><strong>${Number(stats.entry_count)}</strong><span>手账</span></div>
            <div class="stat-item stat-photos"><span class="stat-icon">🖼️</span><strong>${Number(stats.photo_count)}</strong><span>照片</span></div>
            <div class="stat-item stat-comments"><span class="stat-icon">💬</span><strong>${Number(stats.comment_count)}</strong><span>留言</span></div>
          </div>
        </div>
        <aside class="home-aside">
          <section>
            <div class="section-title">
              <h2>即将到来</h2>
              <a class="btn ghost sm" href="#/milestones">全部纪念日</a>
            </div>
            <div class="milestone-grid">${milestoneHtml}</div>
          </section>
        </aside>
      </div>
    `;

    document.querySelector("#random-entry").addEventListener("click", handleRandomEntry);
    document.querySelector("#random-entry-top").addEventListener("click", handleRandomEntry);
  } catch (error) {
    view.innerHTML = emptyState(error.message || "加载失败");
  }
}

async function handleRandomEntry(event) {
  const button = event?.currentTarget || document.querySelector("#random-entry");
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
async function renderSearch(query) {
  view.innerHTML = '<div class="loading">正在翻找这些记忆…</div>';
  const keyword = (query || "").trim();
  try {
    const entries = keyword
      ? await api(`/api/entries?q=${encodeURIComponent(keyword)}`)
      : [];
    const count = entries.length;
    view.innerHTML = `
      <div class="section-title">
        <h2>搜索结果</h2>
        <span class="result-count">${count} 条</span>
      </div>
      <p class="search-keyword">关键词：${esc(keyword || "空")}</p>
      <div class="entry-list">${count ? entries.map((entry) => renderEntryCard(entry)).join("") : emptyState("没有找到相关手账")}</div>
    `;
  } catch (error) {
    view.innerHTML = emptyState(error.message || "搜索失败");
  }
}

function calendarMonthLabel(date) {
async function renderMilestones() {
  view.innerHTML = '<div class="loading">正在整理纪念日…</div>';
  try {
    const milestones = await api("/api/milestones");
    const items = milestoneCountdown(milestones);
    view.innerHTML = `
      <div class="section-title">
        <h2>我们的纪念日</h2>
        <span class="result-count">${items.length} 个</span>
      </div>
      <form id="milestone-form" class="milestone-form card">
        <input id="milestone-name" type="text" placeholder="例如：第一次见面" maxlength="40" required />
        <input id="milestone-date" type="date" required />
        <input id="milestone-emoji" type="text" placeholder="📅" maxlength="16" />
        <label class="milestone-repeat"><input id="milestone-repeat" type="checkbox" checked /> 每年重复</label>
        <button class="btn primary sm" type="submit">添加</button>
      </form>
      <div class="milestone-grid">${items.length ? items.map(milestoneCard).join("") : emptyState("还没有纪念日，添加一个值得记住的日子吧")}</div>
    `;

    document.querySelector("#milestone-form").addEventListener("submit", handleMilestoneCreate);
    document.querySelectorAll("[data-milestone-id]").forEach((card) => {
      card.addEventListener("click", () => handleMilestoneEdit(card.dataset.milestoneId, milestones));
    });
  } catch (error) {
    view.innerHTML = emptyState(error.message || "纪念日加载失败");
  }
}

async function handleMilestoneCreate(event) {
  event.preventDefault();
  const name = document.querySelector("#milestone-name").value.trim();
  const date = document.querySelector("#milestone-date").value;
  const emoji = document.querySelector("#milestone-emoji").value.trim();
  const repeatYearly = document.querySelector("#milestone-repeat").checked ? 1 : 0;
  if (!name || !date) return;
  try {
    await api("/api/milestones", {
      method: "POST",
      body: JSON.stringify({ name, date, emoji, repeat_yearly: repeatYearly }),
    });
    toast("纪念日已添加", "success");
    await renderMilestones();
  } catch (error) {
    toast(error.message || "添加失败");
  }
}

async function handleMilestoneEdit(id, milestones) {
  const milestone = milestones.find((item) => String(item.id) === String(id));
  if (!milestone) return;
  const name = window.prompt("纪念日名称", milestone.name);
  if (name === null) return;
  const date = window.prompt("日期（YYYY-MM-DD）", milestone.date);
  if (date === null) return;
  try {
    await api(`/api/milestones/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: JSON.stringify({ name, date, emoji: milestone.emoji, repeat_yearly: milestone.repeat_yearly }),
    });
    toast("纪念日已更新", "success");
    await renderMilestones();
  } catch (error) {
    toast(error.message || "更新失败");
  }
}
async function handleMilestoneDelete(id) {
  if (!window.confirm("确定删除这个纪念日吗？")) return;
  try {
    await api(`/api/milestones/${encodeURIComponent(id)}`, { method: "DELETE" });
    toast("纪念日已删除", "success");
    await renderMilestones();
  } catch (error) {
    toast(error.message || "删除失败");
  }
}
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
          class="day${has ? " has entry-" + (entry.author === "her" ? "her" : "me") : ""}${isToday ? " today" : ""}"
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
              <div class="photo-caption">${esc(photo.caption || "")}</div>
              <button class="caption-edit" type="button" data-photo-caption="${photo.id}">${photo.caption ? "编辑说明" : "写说明"}</button>
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
async function handlePhotoCaption(photoId) {
  const current = document.querySelector(`[data-photo-caption="${photoId}"]`);
  if (!current) return;
  const caption = window.prompt("给这张照片写一句说明", current.closest(".photo-wrap").querySelector(".photo-caption").textContent);
  if (caption === null) return;
  try {
    await api(`/api/photos/${encodeURIComponent(photoId)}`, {
      method: "PUT",
      body: JSON.stringify({ caption }),
    });
    toast("照片说明已保存", "success");
    const match = /^\/entry\/(\d+)$/.exec(currentRoute());
    if (match) await renderEntryDetail(Number(match[1]));
  } catch (error) {
    toast(error.message || "保存失败");
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
async function handleCommentDelete(commentId) {
  if (!window.confirm("确定删除这条留言吗？")) return;
  try {
    await api(`/api/comments/${encodeURIComponent(commentId)}`, { method: "DELETE" });
    const item = document.querySelector(`[data-comment-id="${commentId}"]`);
    if (item) item.remove();
    toast("留言已删除", "success");
  } catch (error) {
    toast(error.message || "删除失败");
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
  const captionEdit = event.target.closest("[data-photo-caption]");
  if (captionEdit) {
    handlePhotoCaption(captionEdit.dataset.photoCaption);
    return;
  }
  const photo = event.target.closest("[data-photo-url]");
  if (photo) {
    openLightbox(photo.dataset.photoUrl);
    return;
  }

  const commentDelete = event.target.closest("[data-comment-delete]");
  if (commentDelete) {
    handleCommentDelete(commentDelete.dataset.commentDelete);
    return;
  }

  const milestoneDelete = event.target.closest("[data-milestone-delete]");
  if (milestoneDelete) {
    window.handleMilestoneDeletePage(milestoneDelete.dataset.milestoneDelete);
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
themeToggle.addEventListener("click", toggleTheme);
searchForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const keyword = searchInput.value.trim();
  navigate(`#/search/${encodeURIComponent(keyword)}`);
});
loginForm.addEventListener("submit", handleLogin);
window.addEventListener("hashchange", render);
window.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeLightbox();
});

async function boot() {
function nextMilestoneDateGlobal(date, repeatYearly) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date || "");
  if (!match) return null;
  const month = Number(match[2]);
  const day = Number(match[3]);
  const today = new Date();
  const currentYear = today.getFullYear();
  let candidate = new Date(currentYear, month - 1, day);
  if (candidate < new Date(today.getFullYear(), today.getMonth(), today.getDate())) {
    candidate = new Date(currentYear + 1, month - 1, day);
  }
  if (!repeatYearly) {
    candidate = new Date(`${date}T00:00:00`);
  }
  return candidate;
}
function milestoneCountdownGlobal(milestones) {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return milestones
    .map((milestone) => ({ ...milestone, nextDate: nextMilestoneDateGlobal(milestone.date, milestone.repeat_yearly) }))
    .filter((item) => item.nextDate)
    .sort((a, b) => a.nextDate - b.nextDate);
}
function milestoneCardGlobal(milestone) {
  const next = milestone.nextDate || new Date(`${milestone.date}T00:00:00`);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const days = Math.max(0, Math.ceil((next - today) / 86400000));
  const emoji = esc(milestone.emoji || "📅");
  return `
    <article class="milestone-card" data-milestone-id="${milestone.id}">
      <div class="milestone-emoji">${emoji}</div>
      <button class="milestone-del" type="button" data-milestone-delete="${milestone.id}" title="删除纪念日">×</button>
      <div class="milestone-main">
        <strong>${esc(milestone.name)}</strong>
        <span>${formatDate(next.toISOString().slice(0, 10))}</span>
      </div>
      <div class="milestone-days">${days}<small>天</small></div>
    </article>
  `;
}
async function renderMilestonesPage() {
  view.innerHTML = '<div class="loading">正在整理纪念日…</div>';
  try {
    const milestones = await api("/api/milestones");
    const items = milestoneCountdownGlobal(milestones);
    view.innerHTML = `
      <div class="section-title">
        <h2>我们的纪念日</h2>
        <span class="result-count">${items.length} 个</span>
      </div>
      <form id="milestone-form" class="milestone-form card">
        <input id="milestone-name" type="text" placeholder="例如：第一次见面" maxlength="40" required />
        <input id="milestone-date" type="date" required />
        <input id="milestone-emoji" type="text" placeholder="📅" maxlength="16" />
        <label class="milestone-repeat"><input id="milestone-repeat" type="checkbox" checked /> 每年重复</label>
        <button class="btn primary sm" type="submit">添加</button>
      </form>
      <div class="milestone-grid">${items.length ? items.map(milestoneCardGlobal).join("") : emptyState("还没有纪念日，添加一个值得记住的日子吧")}</div>
    `;
    document.querySelector("#milestone-form").addEventListener("submit", handleMilestoneCreatePage);
    document.querySelectorAll("[data-milestone-id]").forEach((card) => {
      card.addEventListener("click", () => handleMilestoneEditPage(card.dataset.milestoneId, milestones));
    });
  } catch (error) {
    view.innerHTML = emptyState(error.message || "纪念日加载失败");
  }
}
async function handleMilestoneCreatePage(event) {
  event.preventDefault();
  const name = document.querySelector("#milestone-name").value.trim();
  const date = document.querySelector("#milestone-date").value;
  const emoji = document.querySelector("#milestone-emoji").value.trim();
  const repeatYearly = document.querySelector("#milestone-repeat").checked ? 1 : 0;
  if (!name || !date) return;
  try {
    await api("/api/milestones", {
      method: "POST",
      body: JSON.stringify({ name, date, emoji, repeat_yearly: repeatYearly }),
    });
    toast("纪念日已添加", "success");
    await renderMilestonesPage();
  } catch (error) {
    toast(error.message || "添加失败");
  }
}
async function handleMilestoneEditPage(id, milestones) {
  const milestone = milestones.find((item) => String(item.id) === String(id));
  if (!milestone) return;
  const name = window.prompt("纪念日名称", milestone.name);
  if (name === null) return;
  const date = window.prompt("日期（YYYY-MM-DD）", milestone.date);
  if (date === null) return;
  try {
    await api(`/api/milestones/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: JSON.stringify({ name, date, emoji: milestone.emoji, repeat_yearly: milestone.repeat_yearly }),
    });
    toast("纪念日已更新", "success");
    await renderMilestonesPage();
  } catch (error) {
    toast(error.message || "更新失败");
  }
}
async function handleMilestoneDeletePage(id) {
  if (!window.confirm("确定删除这个纪念日吗？")) return;
  try {
    await api(`/api/milestones/${encodeURIComponent(id)}`, { method: "DELETE" });
    toast("纪念日已删除", "success");
    await renderMilestonesPage();
  } catch (error) {
    toast(error.message || "删除失败");
  }
}
  window.renderMilestonesPage = renderMilestonesPage;
  window.handleMilestoneDeletePage = handleMilestoneDeletePage;
  const initialTheme = localStorage.getItem(THEME_KEY)
    || (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
  applyTheme(initialTheme);
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
