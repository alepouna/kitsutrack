const { invoke, event } = window.__TAURI__;
const logs = document.querySelector('#logs');
const template = document.querySelector('#entry');

function append(entry) {
  const item = template.content.firstElementChild.cloneNode(true);
  item.classList.add(entry.level.toLowerCase());
  item.querySelector('time').textContent = entry.timestamp;
  item.querySelector('strong').textContent = entry.level;
  item.querySelector('span').textContent = entry.message;
  logs.append(item);
  logs.scrollTop = logs.scrollHeight;
}

invoke('logs').then(entries => entries.forEach(append));
event.listen('log', ({ payload }) => append(payload));
document.querySelector('#export').addEventListener('click', () => invoke('export_logs'));
