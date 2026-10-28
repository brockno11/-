/** Keep the native scrollbar, revealing its thumb briefly after scrolling. */
export function quietScroll(node: HTMLElement) {
  node.classList.add('quiet-scroll')
  let timer: ReturnType<typeof setTimeout> | undefined
  const reveal = () => {
    node.classList.add('is-scrolling')
    clearTimeout(timer)
    timer = setTimeout(() => node.classList.remove('is-scrolling'), 700)
  }
  node.addEventListener('scroll', reveal, { passive: true })
  return { destroy() { clearTimeout(timer); node.removeEventListener('scroll', reveal) } }
}
