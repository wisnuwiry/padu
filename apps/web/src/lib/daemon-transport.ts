import type { HostKind } from '@padu/client'

/**
 * Transports a user can configure from the browser.
 *
 * Tailscale is reachable from a browser because a tailnet address is an
 * ordinary `wss://` URL once the device is on the tailnet. Cloudflare and SSH
 * relays are provisioned by the desktop — they need a tunnel process and a QR
 * handoff — so the web client only displays them.
 */
export const WEB_EDITABLE_KINDS: HostKind[] = ['direct', 'tailscale']

/** Translation key for a transport's short label, or `null` for `direct`. */
export function transportLabelKey(kind: HostKind | undefined): string | null {
  switch (kind) {
    case 'tailscale':
      return 'host.transport_tailscale'
    case 'cloudflare':
      return 'host.transport_cloudflare'
    case 'ssh_relay':
      return 'host.transport_ssh'
    default:
      return null
  }
}
