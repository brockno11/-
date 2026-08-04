const TOOLTIP_DELAY = 400
const VIEWPORT_GAP = 8
const TARGET_GAP = 9

let tooltipElement: HTMLDivElement | undefined
let activeTarget: HTMLElement | undefined
let showTimer: ReturnType<typeof setTimeout> | undefined

function ensureTooltip() {
  if (tooltipElement) return tooltipElement
  tooltipElement = document.createElement('div')
  tooltipElement.id = 'qingjian-tooltip'
  tooltipElement.className = 'app-tooltip'
  tooltipElement.setAttribute('role', 'tooltip')
  document.body.appendChild(tooltipElement)
  return tooltipElement
}

function positionTooltip(target: HTMLElement) {
  const tooltip = ensureTooltip()
  const targetRect = target.getBoundingClientRect()
  const tooltipRect = tooltip.getBoundingClientRect()
  const preferAbove = targetRect.top >= tooltipRect.height + TARGET_GAP + VIEWPORT_GAP
  const top = preferAbove
    ? targetRect.top - tooltipRect.height - TARGET_GAP
    : targetRect.bottom + TARGET_GAP
  const centeredLeft = targetRect.left + targetRect.width / 2 - tooltipRect.width / 2
  const left = Math.max(
    VIEWPORT_GAP,
    Math.min(centeredLeft, window.innerWidth - tooltipRect.width - VIEWPORT_GAP),
  )
  tooltip.style.left = `${Math.round(left)}px`
  tooltip.style.top = `${Math.round(top)}px`
  tooltip.dataset.placement = preferAbove ? 'top' : 'bottom'
}

function hideTooltip() {
  clearTimeout(showTimer)
  showTimer = undefined
  if (activeTarget) activeTarget.removeAttribute('aria-describedby')
  activeTarget = undefined
  tooltipElement?.classList.remove('visible')
}

function showTooltip(target: HTMLElement, content: string) {
  if (!content.trim()) return
  if (activeTarget && activeTarget !== target) {
    activeTarget.removeAttribute('aria-describedby')
  }
  activeTarget = target
  const tooltip = ensureTooltip()
  tooltip.textContent = content
  target.setAttribute('aria-describedby', tooltip.id)
  tooltip.classList.add('visible')
  positionTooltip(target)
}

export function tooltip(node: HTMLElement, initialContent: string) {
  let content = initialContent

  const schedule = () => {
    clearTimeout(showTimer)
    showTimer = setTimeout(() => showTooltip(node, content), TOOLTIP_DELAY)
  }
  const showForKeyboard = () => {
    clearTimeout(showTimer)
    showTooltip(node, content)
  }
  const hideIfActive = () => {
    if (activeTarget === node || showTimer) hideTooltip()
  }

  node.addEventListener('mouseenter', schedule)
  node.addEventListener('mouseleave', hideIfActive)
  node.addEventListener('focus', showForKeyboard)
  node.addEventListener('blur', hideIfActive)
  node.addEventListener('click', hideIfActive)
  window.addEventListener('scroll', hideIfActive, true)
  window.addEventListener('resize', hideIfActive)

  return {
    update(nextContent: string) {
      content = nextContent
      if (activeTarget === node) showTooltip(node, content)
    },
    destroy() {
      node.removeEventListener('mouseenter', schedule)
      node.removeEventListener('mouseleave', hideIfActive)
      node.removeEventListener('focus', showForKeyboard)
      node.removeEventListener('blur', hideIfActive)
      node.removeEventListener('click', hideIfActive)
      window.removeEventListener('scroll', hideIfActive, true)
      window.removeEventListener('resize', hideIfActive)
      hideIfActive()
    },
  }
}

