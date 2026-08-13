const status = document.querySelector('#status');
const iphone = document.querySelector('#iphone');
const rate = document.querySelector('#rate');
const update = document.querySelector('#update');

function render(state) {
  const trackingRate = state.trackingRate ?? state.tracking_rate;
  const updateAvailable = state.updateAvailable ?? state.update_available;
  status.lastChild.textContent = state.status;
  status.className = state.status.includes('Tracking') && !state.status.includes('Waiting') ? '' : state.status.includes('Waiting') ? 'waiting' : 'disconnected';
  iphone.textContent = state.status === 'Disconnected' ? 'Not connected' : state.iphone;
  rate.textContent = trackingRate ? `${trackingRate} FPS` : '—';
  rate.classList.toggle('rate', Boolean(trackingRate));
  update.classList.toggle('hidden', !updateAvailable);
}

function connect() {
  const tauri = window.__TAURI__;
  if (!tauri) {
    setTimeout(connect, 25);
    return;
  }
  tauri.core.invoke('menu_state').then(render);
  tauri.event.listen('menu-state', ({ payload }) => render(payload));
  document.querySelectorAll('[data-command]').forEach(button => {
    button.addEventListener('click', () => tauri.core.invoke(button.dataset.command));
  });
}

connect();
