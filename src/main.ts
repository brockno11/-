import { mount } from 'svelte'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './app.css'

async function start() {
  const previewWindow = new URLSearchParams(window.location.search).get('window')
  const label = '__TAURI_INTERNALS__' in window ? getCurrentWindow().label : previewWindow || 'hub'
  const Component = label === 'editor'
    ? (await import('./EditorWindow.svelte')).default
    : (await import('./Hub.svelte')).default
  mount(Component, { target: document.getElementById('app')! })
}

void start()
