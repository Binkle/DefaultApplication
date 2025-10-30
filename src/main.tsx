import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import '../styles/app.css';
window.addEventListener('scroll', () => {
  const btn = document.querySelector<HTMLButtonElement>('.scroll-top');
  if (!btn) return;
  const show = window.scrollY > 160;
  btn.style.opacity = show ? '1' : '0';
  btn.style.pointerEvents = show ? 'auto' : 'none';
});

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
