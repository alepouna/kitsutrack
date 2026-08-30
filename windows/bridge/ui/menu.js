const status = document.querySelector('#status');
const iphone = document.querySelector('#iphone');
const rate = document.querySelector('#rate');
const checkUpdates = document.querySelector('#check-updates');
const checkUpdatesLabel = checkUpdates.querySelector('span');
const refreshIcon = document.querySelector('#refresh-icon');
const downloadIcon = document.querySelector('#download-icon');
let invoke, reportingError = false;

function reportError(error, context = 'Menu UI') {
  const message = `${context}: ${error instanceof Error ? error.message : String(error)}`;
  status.lastChild.textContent = 'UI error — bridge still running';
  status.className = 'disconnected';
  document.querySelector('#retry').classList.remove('hidden');
  if (invoke && !reportingError) {
    reportingError = true;
    invoke('client_error', { message }).catch(() => {}).finally(() => { reportingError = false; });
  }
}

function render(state) {
  document.querySelector('#retry').classList.add('hidden');
  const trackingRate = state.trackingRate ?? state.tracking_rate;
  const updateAvailable = state.updateAvailable ?? state.update_available;
  status.lastChild.textContent = state.status;
  status.className = state.status.includes('Tracking') && !state.status.includes('Waiting') ? '' : state.status.includes('Waiting') ? 'waiting' : 'disconnected';
  iphone.textContent = state.status === 'Disconnected' ? 'Not connected' : state.iphone;
  rate.textContent = trackingRate ? `${trackingRate} FPS` : 'Not connected';
  rate.classList.toggle('rate', Boolean(trackingRate));
  checkUpdatesLabel.textContent = updateAvailable ? 'Updates Available' : 'Check for Updates';
  checkUpdates.dataset.command = updateAvailable ? 'open_update_command' : 'check_for_updates';
  checkUpdates.classList.toggle('updates-available', updateAvailable);
  refreshIcon.classList.toggle('hidden', updateAvailable);
  downloadIcon.classList.toggle('hidden', !updateAvailable);
}

function connect() {
  const tauri = window.__TAURI__;
  if (!tauri) {
    setTimeout(connect, 25);
    return;
  }
  invoke = tauri.core.invoke;
  invoke('menu_state').then(render).catch(error => reportError(error, 'Menu command menu_state'));
  tauri.event.listen('menu-state', ({ payload }) => render(payload)).catch(error => reportError(error, 'Menu event listener'));
  document.querySelectorAll('[data-command]').forEach(button => {
    button.addEventListener('click', () => invoke(button.dataset.command).catch(error => reportError(error, `Menu command ${button.dataset.command}`)));
  });
}

window.addEventListener('error', event => reportError(event.error || event.message, 'Menu UI error'));
window.addEventListener('unhandledrejection', event => reportError(event.reason, 'Menu UI rejection'));
connect();
