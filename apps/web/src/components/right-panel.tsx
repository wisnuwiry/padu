// Public compatibility facade. Keep importing `@/components/right-panel` stable while
// the implementation is organized by panel concern under `right-panel/`.
export {
  PanelTabButton,
  PanelTabStrip,
  RightPanel,
  panelContentId,
  panelTabId,
} from './right-panel/index'
export type {
  PanelSurface,
  PanelTab,
  PanelTabStripProps,
  RightPanelHandle,
  RightPanelProps,
} from './right-panel/index'
