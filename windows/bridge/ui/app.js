const $ = selector => document.querySelector(selector);
let invoke, files = [], entries = [], currentSession = null, selectedLevel = 'all', reportingError = false;
const formatDate = value => { const date = new Date(Number(value) || value); return Number.isNaN(date.valueOf()) ? value : date.toLocaleString(); };
const formatSize = bytes => bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(1)} KB` : `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
function errorMessage(error) { return error instanceof Error ? error.message : String(error); }
function reportError(error, context = 'Logs UI') {
  const message = `${context}: ${errorMessage(error)}`;
  $('#summary').textContent = message;
  $('#retry').hidden = false;
  if (invoke && !reportingError) {
    reportingError = true;
    invoke('client_error', { message }).catch(() => {}).finally(() => { reportingError = false; });
  }
}
async function call(command, args) {
  try { return await invoke(command, args); }
  catch (error) { reportError(error, `Logs command ${command}`); throw error; }
}
function showEmpty(show, detail = false) { $('#empty').hidden = !show; $('#empty h2').textContent = detail ? 'No matching lines' : 'No launch logs'; $('#empty p').textContent = detail ? 'Try changing the search or severity filter.' : 'New bridge launches will appear here automatically.'; }
function renderFiles() {
  $('#files').replaceChildren(); $('#files').hidden = false; $('#entries').hidden = true; $('#toolbar').hidden = true; $('#file-actions').hidden = true; $('#main-actions').hidden = false; $('#back').hidden = true; $('#retry').hidden = true;
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
    card.querySelector('.reveal').onclick = () => call('reveal_log_file', { session: file.session }).catch(() => {});
    card.querySelector('.delete').onclick = () => deleteFile(file.session);
    $('#files').append(card);
  }
  showEmpty(files.length === 0);
}
async function openFile(session) {
  const file = files.find(item => item.session === session);
  if (file && file.size > 800 * 1024 && !window.confirm('Opening this log file may take a moment, and the log viewer may be unresponsive. Continue?')) return;
  $('#summary').textContent = 'Opening log…';
  try {
    currentSession = session; entries = await call('log_file', { session });
  } catch (_) { return; }
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
  try {
    await call('delete_log_file', { session }); files = await call('log_files'); currentSession = null; renderFiles();
  } catch (_) {}
}
async function deleteAll() {
  if (!await confirmDelete('Delete all launch logs?', 'This permanently removes every saved KitsuTrack Bridge log file. This cannot be undone.')) return;
  try {
    await call('delete_all_log_files'); files = await call('log_files'); currentSession = null; renderFiles();
  } catch (_) {}
}
function connect() {
  if (!window.__TAURI__) return setTimeout(connect, 25); invoke = window.__TAURI__.core.invoke;
  call('log_files').then(result => { files = result; renderFiles(); }).catch(() => {});
  window.__TAURI__.event.listen('log', ({ payload }) => { if (currentSession === payload.session) { entries.push(payload); renderEntries(); } }).catch(error => reportError(error, 'Logs event listener'));
}
async function retryLoadFiles() { try { files = await call('log_files'); renderFiles(); } catch (_) {} }
$('#back').onclick = renderFiles; $('#retry').onclick = retryLoadFiles; $('#export-all').onclick = () => call('export_all_logs').catch(() => {}); $('#delete-all').onclick = deleteAll; $('#reveal-file').onclick = () => call('reveal_log_file', { session: currentSession }).catch(() => {}); $('#delete-file').onclick = () => deleteFile(currentSession); $('#search').oninput = renderEntries;
document.querySelectorAll('[data-level]').forEach(button => button.onclick = () => { $('.filters .active').classList.remove('active'); button.classList.add('active'); selectedLevel = button.dataset.level; renderEntries(); });
window.addEventListener('error', event => reportError(event.error || event.message, 'Logs UI error'));
window.addEventListener('unhandledrejection', event => reportError(event.reason, 'Logs UI rejection'));
connect();
