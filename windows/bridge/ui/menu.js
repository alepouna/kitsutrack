const { invoke, event } = window.__TAURI__;
const status = document.querySelector('#status');
const update = document.querySelector('#update');

function render(state) {
  status.textContent = state.status;
  update.classList.toggle('hidden', !state.updateAvailable);
}

invoke('menu_state').then(render);
event.listen('menu-state', ({ payload }) => render(payload));
document.querySelectorAll('[data-command]').forEach(button => {
  button.addEventListener('click', () => invoke(button.dataset.command));
});
