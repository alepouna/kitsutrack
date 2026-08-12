const { invoke, event } = window.__TAURI__;
const status = document.querySelector('#status');
const iphone = document.querySelector('#iphone');
const rate = document.querySelector('#rate');
const update = document.querySelector('#update');

function render(state) {
  status.lastChild.textContent = state.status;
  status.className = state.status.includes('Tracking') && !state.status.includes('Waiting') ? '' : state.status.includes('Waiting') ? 'waiting' : 'disconnected';
  iphone.textContent = state.status === 'Disconnected' ? 'Not connected' : state.iphone;
  rate.textContent = state.trackingRate ? `${state.trackingRate} FPS` : '—';
  rate.classList.toggle('rate', Boolean(state.trackingRate));
  update.classList.toggle('hidden', !state.updateAvailable);
}

invoke('menu_state').then(render);
event.listen('menu-state', ({ payload }) => render(payload));
document.querySelectorAll('[data-command]').forEach(button => button.addEventListener('click', () => invoke(button.dataset.command)));
