"use strict";

const MAX_VOICE_BYTES = 20 * 1024 * 1024;

function canRecordAudio() {
  return Boolean(
    window.isSecureContext
      && navigator.mediaDevices?.getUserMedia
      && window.MediaRecorder,
  );
}

function recordingUnavailableMessage() {
  if (!window.isSecureContext) {
    return "手机浏览器需要 HTTPS 才能录音，可先使用“上传音频”";
  }
  return "当前浏览器不支持录音，可先使用“上传音频”";
}

function microphoneErrorMessage(error) {
  if (!window.isSecureContext) return recordingUnavailableMessage();
  if (error?.name === "NotAllowedError") return "麦克风权限未开启，请在浏览器设置中允许访问";
  if (error?.name === "NotFoundError") return "没有检测到可用的麦克风，请改用“上传音频”";
  return "无法访问麦克风，请检查浏览器权限或改用“上传音频”";
}

function prepareRecordButton(button) {
  if (canRecordAudio()) return true;
  button.textContent = "录音需 HTTPS";
  button.title = recordingUnavailableMessage();
  return false;
}

function reviewPhoto(photo) {
  return `
    <figure class="review-photo">
      <img src="${photoUrl(photo)}" alt="${esc(photo.original_name)}" loading="lazy" />
      ${photo.caption ? `<figcaption>${esc(photo.caption)}</figcaption>` : ""}
    </figure>
  `;
}

function reviewLetterEcho(letter) {
  const voices = letter.voice_notes?.length
    ? `<div class="review-letter-voices">${letter.voice_notes.map((voice) => `<audio controls preload="metadata" src="/letter-voice/${encodeURIComponent(voice.filename)}"></audio>`).join("")}</div>`
    : "";
  return `
    <article class="review-letter">
      <span class="letter-label">${formatDate(letter.open_at)} · ${esc(letter.author)}</span>
      <h3>${esc(letter.title)}</h3>
      <p>${esc(letter.content || "")}</p>
      ${voices}
    </article>
  `;
}

async function renderReviewPage() {
  view.innerHTML = '<div class="loading">正在把这一年的片段装订成册…</div>';
  try {
    const data = await api("/api/review");
    const maxCount = Math.max(1, ...data.monthly.map((item) => item.count));
    const months = data.monthly.length
      ? data.monthly.map((item) => `
          <div class="review-month">
            <div class="review-month-label">${formatMonth(item.month)}</div>
            <div class="review-month-track"><span style="width:${(item.count / maxCount) * 100}%"></span></div>
            <strong>${item.count}</strong>
          </div>
        `).join("")
      : emptyState("还没有足够的数据生成年度回顾");
    const photos = data.recent_photos.length
      ? data.recent_photos.map(reviewPhoto).join("")
      : emptyState("照片会在这里慢慢长大");
    const first = data.first_entry
      ? `<div class="review-quote"><span>第一篇</span><p>${esc(data.first_entry.note)}</p><small>${formatDate(data.first_entry.date)}</small></div>`
      : "";
    const latest = data.latest_entry
      ? `<div class="review-quote"><span>最近一篇</span><p>${esc(data.latest_entry.note)}</p><small>${formatDate(data.latest_entry.date)}</small></div>`
      : "";
    const letters = data.unlocked_letters?.length
      ? data.unlocked_letters.map(reviewLetterEcho).join("")
      : emptyState("这一年还没有解封的时间胶囊");

    view.innerHTML = `
      <section class="review-hero">
        <span class="eyebrow">ONE YEAR TOGETHER</span>
        <h1>我们的一年，<br />被认真记住的每一天</h1>
        <p>这不是一份报告，是你们共同生活留下的温度。</p>
      </section>
      <section class="review-stats">
        <div><strong>${data.days_together}</strong><span>在一起的天数</span></div>
        <div><strong>${data.entry_count}</strong><span>篇手账</span></div>
        <div><strong>${data.photo_count}</strong><span>张照片</span></div>
        <div><strong>${data.voice_count}</strong><span>段声音</span></div>
        <div><strong>${data.total_words}</strong><span>个字被写下</span></div>
      </section>
      <section class="card review-chapter">
        <div class="section-title"><h2>十二个月的痕迹</h2><span class="result-count">${data.busiest_month ? `记录最多：${formatMonth(data.busiest_month)}` : ""}</span></div>
        <div class="review-months">${months}</div>
      </section>
      <section class="review-two">
        ${first}
        ${latest}
      </section>
      <section class="card">
        <div class="section-title"><h2>照片装订线</h2><span class="result-count">${data.photo_count} 张回忆</span></div>
        <div class="review-photos">${photos}</div>
      </section>
      <section class="card review-letter-chapter">
        <div class="section-title"><h2>时间胶囊回声</h2><span class="result-count">${data.unlocked_letters?.length || 0} 封已解封</span></div>
        <div class="review-letters">${letters}</div>
      </section>
    `;
  } catch (error) {
    view.innerHTML = emptyState(error.message || "年度回顾加载失败");
  }
}

function letterVoiceTemplate(note) {
  return `
    <div class="voice-item" data-letter-voice-id="${note.id}">
      <div class="voice-icon">♪</div>
      <div class="voice-main">
        <strong>${esc(note.original_name || "封存的声音")}</strong>
        <audio controls preload="metadata" src="/letter-voice/${encodeURIComponent(note.filename)}"></audio>
      </div>
      <button class="voice-del" type="button" data-letter-voice-delete="${note.id}" title="删除">×</button>
    </div>
  `;
}

function letterVoiceControls(letter) {
  return `
    <div class="letter-voice" data-letter-voice="${letter.id}">
      <div class="voice-list">
        ${letter.unlocked && letter.voice_notes?.length ? letter.voice_notes.map(letterVoiceTemplate).join("") : `<div class="empty voice-empty">${letter.unlocked ? "还没有封存声音" : `已封存 ${letter.voice_count || 0} 段声音，到期后解锁`}</div>`}
      </div>
      <div class="voice-recorder">
        <button class="btn primary sm" type="button" data-letter-record="${letter.id}">开始录音</button>
        <label class="upload-button">补录音频<input type="file" accept="audio/*" hidden data-letter-voice-file="${letter.id}" /></label>
      </div>
    </div>
  `;
}

function letterCard(letter) {
  if (!letter.unlocked) {
    return `
      <article class="letter-card locked">
        <div class="letter-seal">✦</div>
        <span class="letter-label">${formatDate(letter.open_at)} 开启</span>
        <h3>${esc(letter.title)}</h3>
        <p>这是一封还没有到时间的信。</p>
        <strong>还有 ${letter.days_until} 天</strong>
        ${letterVoiceControls(letter)}
        <button class="btn danger sm" type="button" data-letter-delete="${letter.id}">删除</button>
      </article>
    `;
  }
  return `
    <article class="letter-card opened">
      <span class="letter-label">${formatDate(letter.open_at)} · ${esc(letter.author)}</span>
      <h3>${esc(letter.title)}</h3>
      <div class="letter-content">${esc(letter.content || "")}</div>
      ${letterVoiceControls(letter)}
      <button class="btn danger sm" type="button" data-letter-delete="${letter.id}">删除</button>
    </article>
  `;
}

async function renderLettersPage() {
  view.innerHTML = '<div class="loading">正在打开时间抽屉…</div>';
  try {
    const letters = await api("/api/letters");
    const cards = letters.length ? letters.map(letterCard).join("") : emptyState("还没有未来信，写下第一封吧");
    const today = new Date();
    today.setDate(today.getDate() + 365);
    const defaultDate = dateKey(today.getFullYear(), today.getMonth(), today.getDate());
    view.innerHTML = `
      <div class="section-title">
        <h2>未来信与时间胶囊</h2>
        <span class="result-count">${letters.length} 封</span>
      </div>
      <form id="letter-form" class="letter-form card">
        <input id="letter-title" type="text" maxlength="80" placeholder="信的名字，例如：给明年的我们" required />
        <textarea id="letter-content" maxlength="5000" placeholder="写下此刻想对未来的我们说的话…" required></textarea>
        <div class="letter-form-row">
          <input id="letter-author" type="text" maxlength="20" placeholder="署名" required />
          <label>开启日期 <input id="letter-date" type="date" value="${defaultDate}" required /></label>
          <button class="btn primary" type="submit">封存这封信</button>
        </div>
      </form>
      <div class="letter-grid">${cards}</div>
    `;
    document.querySelector("#letter-form").addEventListener("submit", handleLetterCreate);
    document.querySelectorAll("[data-letter-delete]").forEach((button) => {
      button.addEventListener("click", () => handleLetterDelete(button.dataset.letterDelete));
    });
    document.querySelectorAll("[data-letter-voice]").forEach((container) => {
      initLetterVoice(container);
    });
  } catch (error) {
    view.innerHTML = emptyState(error.message || "未来信加载失败");
  }
}

async function handleLetterCreate(event) {
  event.preventDefault();
  const payload = {
    title: document.querySelector("#letter-title").value.trim(),
    content: document.querySelector("#letter-content").value.trim(),
    author: document.querySelector("#letter-author").value.trim(),
    open_at: document.querySelector("#letter-date").value,
  };
  if (!payload.title || !payload.content || !payload.author || !payload.open_at) return;
  try {
    await api("/api/letters", { method: "POST", body: JSON.stringify(payload) });
    toast("这封信已经封存", "success");
    await renderLettersPage();
  } catch (error) {
    toast(error.message || "封存失败");
  }
}

async function handleLetterDelete(id) {
  if (!window.confirm("确定删除这封信吗？")) return;
  try {
    await api(`/api/letters/${encodeURIComponent(id)}`, { method: "DELETE" });
    toast("信已删除", "success");
    await renderLettersPage();
  } catch (error) {
    toast(error.message || "删除失败");
  }
}

function voiceTemplate(note) {
  return `
    <div class="voice-item" data-voice-id="${note.id}">
      <div class="voice-icon">♪</div>
      <div class="voice-main">
        <strong>${esc(note.original_name || "声音回忆")}</strong>
        <audio controls preload="metadata" src="/voice/${encodeURIComponent(note.filename)}"></audio>
      </div>
      <button class="voice-del" type="button" data-voice-delete="${note.id}" title="删除">×</button>
    </div>
  `;
}

function initVoiceSection(entryId, notes) {
  const list = document.querySelector("#voice-list");
  const recordButton = document.querySelector("#voice-record");
  const fileInput = document.querySelector("#voice-file");
  if (!list || !recordButton || !fileInput) return;
  prepareRecordButton(recordButton);
  let mediaRecorder = null;
  let chunks = [];
  let startedAt = 0;

  list.innerHTML = notes.length
    ? notes.map(voiceTemplate).join("")
    : '<div class="empty voice-empty">还没有声音，录下一段此刻吧</div>';

  list.querySelectorAll("[data-voice-delete]").forEach((button) => {
    button.addEventListener("click", async () => {
      if (!window.confirm("确定删除这段声音吗？")) return;
      try {
        await api(`/api/voice/${encodeURIComponent(button.dataset.voiceDelete)}`, { method: "DELETE" });
        button.closest(".voice-item").remove();
        toast("声音已删除", "success");
      } catch (error) {
        toast(error.message || "删除失败");
      }
    });
  });

  async function uploadVoice(blob, name, duration) {
    const formData = new FormData();
    formData.append("audio", blob, name);
    formData.append("duration", String(duration));
    try {
      const note = await api(`/api/entries/${entryId}/voice`, { method: "POST", body: formData });
      list.querySelector(".voice-empty")?.remove();
      list.insertAdjacentHTML("beforeend", voiceTemplate(note));
      const justAdded = list.querySelector(`[data-voice-id="${note.id}"] [data-voice-delete]`);
      justAdded?.addEventListener("click", async () => {
        if (!window.confirm("确定删除这段声音吗？")) return;
        await api(`/api/voice/${encodeURIComponent(note.id)}`, { method: "DELETE" });
        justAdded.closest(".voice-item").remove();
      });
      toast("声音已经收好", "success");
    } catch (error) {
      toast(error.message || "语音上传失败");
    }
  }

  recordButton.addEventListener("click", async () => {
    if (mediaRecorder?.state === "recording") {
      mediaRecorder.stop();
      return;
    }
    if (!canRecordAudio()) {
      toast(recordingUnavailableMessage());
      return;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      chunks = [];
      mediaRecorder = new MediaRecorder(stream);
      startedAt = Date.now();
      mediaRecorder.ondataavailable = (event) => event.data.size && chunks.push(event.data);
      mediaRecorder.onstop = async () => {
        stream.getTracks().forEach((track) => track.stop());
        const duration = (Date.now() - startedAt) / 1000;
        const blob = new Blob(chunks, { type: mediaRecorder.mimeType || "audio/webm" });
        await uploadVoice(blob, `voice-${Date.now()}.webm`, duration);
        recordButton.textContent = "开始录音";
        recordButton.classList.remove("recording");
      };
      mediaRecorder.start();
      recordButton.textContent = "停止录音";
      recordButton.classList.add("recording");
    } catch (error) {
      toast(microphoneErrorMessage(error));
    }
  });

  fileInput.addEventListener("change", async () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    await uploadVoice(file, file.name, 0);
    fileInput.value = "";
  });
}

const LETTER_NOTIFIED_KEY = "love_journal_notified_letters";

async function checkLetterNotifications() {
  const app = document.querySelector("#app");
  if (!app || app.hidden) return;
  try {
    const due = await api("/api/letters/notifications");
    updateLetterReminder(due);
    notifyBrowser(due);
  } catch {
    // The global API handler already deals with expired sessions.
  }
}

function updateLetterReminder(due) {
  const reminder = document.querySelector("#letter-reminder");
  const badge = document.querySelector("#letter-badge");
  if (!reminder || !badge) return;
  if (!due.length) {
    reminder.hidden = true;
    badge.hidden = true;
    return;
  }
  badge.hidden = false;
  badge.textContent = String(due.length);
  const titles = due.map((item) => esc(item.title)).join("、");
  reminder.hidden = false;
  reminder.innerHTML = `
    <div class="letter-reminder-icon">✦</div>
    <div class="letter-reminder-copy">
      <strong>时间胶囊到期了</strong>
      <span>${due.length} 封信可以打开：${titles}</span>
    </div>
    <div class="letter-reminder-actions">
      <button id="letter-enable-notify" class="btn ghost sm" type="button">开启浏览器提醒</button>
      <button id="letter-open-due" class="btn primary sm" type="button">去查看</button>
    </div>
  `;

  const notifyButton = document.querySelector("#letter-enable-notify");
  if (!("Notification" in window) || Notification.permission === "granted") {
    notifyButton.hidden = true;
  } else if (Notification.permission === "denied") {
    notifyButton.disabled = true;
    notifyButton.textContent = "浏览器通知已关闭";
  } else {
    notifyButton.addEventListener("click", async () => {
      const permission = await Notification.requestPermission();
      if (permission === "granted") {
        toast("浏览器提醒已开启", "success");
        notifyButton.hidden = true;
      }
    });
  }

  document.querySelector("#letter-open-due").addEventListener("click", async () => {
    try {
      await Promise.all(due.map((letter) => api(`/api/letters/${letter.id}/open`, { method: "POST" })));
      localStorage.setItem(LETTER_NOTIFIED_KEY, JSON.stringify([]));
      reminder.hidden = true;
      badge.hidden = true;
      navigate("#/letters");
    } catch (error) {
      toast(error.message || "打开失败");
    }
  });
}

function notifyBrowser(due) {
  if (!("Notification" in window) || Notification.permission !== "granted") return;
  let notified = [];
  try { notified = JSON.parse(localStorage.getItem(LETTER_NOTIFIED_KEY) || "[]"); } catch { notified = []; }
  const newItems = due.filter((item) => !notified.includes(item.id));
  newItems.forEach((item) => {
    const notification = new Notification("时间胶囊可以打开了", {
      body: item.title,
      tag: `letter-${item.id}`,
    });
    notification.onclick = () => {
      window.focus();
      navigate("#/letters");
    };
  });
  localStorage.setItem(LETTER_NOTIFIED_KEY, JSON.stringify([...new Set([...notified, ...due.map((item) => item.id)])]));
}

window.checkLetterNotifications = checkLetterNotifications;
window.renderReviewPage = renderReviewPage;
function initLetterVoice(container) {
  const letterId = container.dataset.letterVoice;
  const list = container.querySelector(".voice-list");
  const recordButton = container.querySelector("[data-letter-record]");
  const fileInput = container.querySelector("[data-letter-voice-file]");
  if (!list || !recordButton || !fileInput) return;
  prepareRecordButton(recordButton);
  let mediaRecorder = null;
  let chunks = [];
  let startedAt = 0;

  container.querySelectorAll("[data-letter-voice-delete]").forEach((button) => {
    button.addEventListener("click", async () => {
      if (!window.confirm("确定删除这段封存声音吗？")) return;
      try {
        await api(`/api/letter-voice/${encodeURIComponent(button.dataset.letterVoiceDelete)}`, { method: "DELETE" });
        button.closest(".voice-item").remove();
      } catch (error) {
        toast(error.message || "删除失败");
      }
    });
  });

  async function upload(blob, name, duration) {
    const formData = new FormData();
    formData.append("audio", blob, name);
    formData.append("duration", String(duration));
    try {
      const note = await api(`/api/letters/${letterId}/voice`, { method: "POST", body: formData });
      list.querySelector(".voice-empty")?.remove();
      list.insertAdjacentHTML("beforeend", letterVoiceTemplate(note));
      const justAdded = list.querySelector(`[data-letter-voice-id="${note.id}"] [data-letter-voice-delete]`);
      justAdded?.addEventListener("click", async () => {
        if (!window.confirm("确定删除这段封存声音吗？")) return;
        try {
          await api(`/api/letter-voice/${encodeURIComponent(note.id)}`, { method: "DELETE" });
          justAdded.closest(".voice-item").remove();
        } catch (error) {
          toast(error.message || "删除失败");
        }
      });
      toast("声音已经封存", "success");
    } catch (error) {
      toast(error.message || "上传失败");
    }
  }

  recordButton.addEventListener("click", async () => {
    if (mediaRecorder?.state === "recording") {
      mediaRecorder.stop();
      return;
    }
    if (!canRecordAudio()) {
      toast(recordingUnavailableMessage());
      return;
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      chunks = [];
      mediaRecorder = new MediaRecorder(stream);
      startedAt = Date.now();
      mediaRecorder.ondataavailable = (event) => event.data.size && chunks.push(event.data);
      mediaRecorder.onstop = async () => {
        stream.getTracks().forEach((track) => track.stop());
        const duration = (Date.now() - startedAt) / 1000;
        const type = mediaRecorder.mimeType?.split(";")[0] || "audio/webm";
        await upload(new Blob(chunks, { type }), `letter-${Date.now()}.webm`, duration);
        recordButton.textContent = "开始录音";
        recordButton.classList.remove("recording");
      };
      mediaRecorder.start();
      recordButton.textContent = "停止录音";
      recordButton.classList.add("recording");
    } catch (error) {
      toast(microphoneErrorMessage(error));
    }
  });

  fileInput.addEventListener("change", async () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    if (file.size > MAX_VOICE_BYTES) {
      toast("语音文件不能超过 20 MB");
      fileInput.value = "";
      return;
    }
    await upload(file, file.name, 0);
    fileInput.value = "";
  });
}

window.renderLettersPage = renderLettersPage;
window.initVoiceSection = initVoiceSection;
