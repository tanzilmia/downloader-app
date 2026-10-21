import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource-variable/manrope';
import '@fontsource/dm-mono/400.css';
import './styles.css';
import { App } from './app';
import { initializeApp, useApp } from './state/app';

window.addEventListener('error', (event) => {
  useApp.setState({ error: event.message || 'An unexpected error occurred.' });
});
void initializeApp();
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
