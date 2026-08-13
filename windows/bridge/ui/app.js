const $ = selector => document.querySelector(selector);
let invoke, files = [], entries = [], currentSession = null, selectedLevel = 'all';
const formatDate = value => { const date = new Date(Number(value) || value); return Number.isNaN(date.valueOf()) ? value : date.toLocaleString(); };
const formatSize = bytes => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
function showEmpty(show, detail = false) { $('#empty').hidden = !show; $('#empty h2').textContent = detail ? 'No matching lines' : 'No launch logs'; $('#empty p').textContent = detail ? 'Try changing the search or severity filter.' : 'New bridge launches will appear here automatically.'; }
function renderFiles() {
  $('#files').replaceChildren(); $('#files').hidden = false; $('#entries').hidden = true; $('#toolbar').hidden = true; $('#file-actions').hidden = true; $('#main-actions').hidden = false; $('#back').hidden = true;
  $('#title').textContent = 'KitsuTrack Bridge Logs'; $('#summary').textContent = `${files.length} of the last 10 launches`;
  for (const file of files) {
    const card = $('#file-template').content.firstElementChild.cloneNode(true);
    card.querySelector('strong').textContent = `${formatDate(file.startedAt)}${file.current ? ' · Current launch' : ''}`;
    card.querySelector('.file-meta').textContent = `${file.entries} ${file.entries === 1 ? 'line' : 'lines'} · ${formatSize(file.size)}`;
    const severity = card.querySelector('.severity');
    if (file.errors) severity.innerHTML += `<span class="error-pill">${file.errors} ${file.errors === 1 ? 'error' : 'errors'}</span>`;
    if (file.warnings) severity.innerHTML += `<span class="warning-pill">${file.warnings} ${file.warnings === 1 ? 'warning' : 'warnings'}</span>`;
    if (!file.errors && !file.warnings) severity.innerHTML = '<span class="ok-pill">No issues</span>';
    card.querySelector('.file-open').onclick = () => openFile(file.session);
    card.querySelector('.reveal').onclick = () => invoke('reveal_log_file', { session: file.session });
    card.querySelector('.delete').onclick = () => deleteFile(file.session);
    $('#files').append(card);
  }
  showEmpty(files.length === 0);
}
async function openFile(session) {
  currentSession = session; entries = await invoke('log_file', { session });
  $('#files').hidden = true; $('#entries').hidden = false; $('#toolbar').hidden = false; $('#file-actions').hidden = false; $('#main-actions').hidden = true; $('#back').hidden = false;
  $('#title').textContent = formatDate(session); renderEntries();
}
function renderEntries() {
  const query = $('#search').value.trim().toLowerCase();
  const visible = entries.filter(e => (selectedLevel === 'all' || e.level === selectedLevel) && (!query || e.message.toLowerCase().includes(query)));
  $('#entries').replaceChildren(); $('#summary').textContent = `${entries.length} ${entries.length === 1 ? 'line' : 'lines'}`;
  for (const line of visible) {
    const row = $('#entry-template').content.firstElementChild.cloneNode(true); row.classList.add(line.level);
    row.querySelector('.level').textContent = `[${line.level}]`; row.querySelector('time').textContent = `[${formatDate(line.timestamp)}]`; row.querySelector('p').textContent = line.message;
    row.querySelector('.copy').onclick = async event => { await navigator.clipboard.writeText(`${line.timestamp} ${line.level.toUpperCase()} ${line.message}`); event.currentTarget.classList.add('copied'); setTimeout(() => event.currentTarget.classList.remove('copied'), 800); };
    $('#entries').append(row);
  }
  showEmpty(visible.length === 0, true);
}
function confirmDelete(title, message) {
  return new Promise(resolve => {
    const modal = $('#confirm-modal'); $('#modal-title').textContent = title; $('#modal-message').textContent = message;
    const finish = value => { modal.close(); resolve(value); };
    $('#cancel-delete').onclick = () => finish(false); $('#confirm-delete').onclick = () => finish(true);
    modal.oncancel = event => { event.preventDefault(); finish(false); }; modal.showModal();
  });
}
async function deleteFile(session) {
  if (!await confirmDelete('Delete this launch log?', 'This permanently removes the complete log file from disk. This cannot be undone.')) return;
  await invoke('delete_log_file', { session }); files = await invoke('log_files'); currentSession = null; renderFiles();
}
async function deleteAll() {
  if (!await confirmDelete('Delete all launch logs?', 'This permanently removes every saved KitsuTrack Bridge log file. This cannot be undone.')) return;
  await invoke('delete_all_log_files'); files = await invoke('log_files'); currentSession = null; renderFiles();
}
function connect() {
  if (!window.__TAURI__) return setTimeout(connect, 25); invoke = window.__TAURI__.core.invoke;
  invoke('log_files').then(result => { files = result; renderFiles(); });
  window.__TAURI__.event.listen('log', ({ payload }) => { if (currentSession === payload.session) { entries.push(payload); renderEntries(); } });
}
$('#back').onclick = renderFiles; $('#export-all').onclick = () => invoke('export_all_logs'); $('#delete-all').onclick = deleteAll; $('#reveal-file').onclick = () => invoke('reveal_log_file', { session: currentSession }); $('#delete-file').onclick = () => deleteFile(currentSession); $('#search').oninput = renderEntries;
document.querySelectorAll('[data-level]').forEach(button => button.onclick = () => { $('.filters .active').classList.remove('active'); button.classList.add('active'); selectedLevel = button.dataset.level; renderEntries(); });
connect();
