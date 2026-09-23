import { describe, expect, test } from 'bun:test';

import {
  classifyConnection,
  displayHost,
  isPrivateDaemonAddress,
  normalizeDaemonAddress,
  normalizeDaemonProfile,
  parseDaemonProfiles,
  profileInitials,
} from './daemon-profile';

describe('daemon profiles', () => {
  test('normalizes host, HTTP aliases, and protocol paths', () => {
    expect(normalizeDaemonAddress('padu.local:34123')).toBe('ws://padu.local:34123');
    expect(normalizeDaemonAddress('https://padu.example.com/v1?old=1')).toBe(
      'wss://padu.example.com',
    );
  });

  test('rejects unsupported schemes and embedded credentials', () => {
    expect(() => normalizeDaemonAddress('ftp://padu.local')).toThrow('ws:// or wss://');
    expect(() => normalizeDaemonAddress('ws://user:secret@padu.local')).toThrow('no credentials');
  });

  test('derives a useful default name without losing timestamps', () => {
    const profile = normalizeDaemonProfile(
      { name: '', address: 'wss://work.example.com', token: 'secret' },
      undefined,
      'daemon-id',
      100,
    );
    expect(profile).toEqual({
      id: 'daemon-id',
      name: 'work.example.com',
      address: 'wss://work.example.com',
      kind: 'direct',
      createdAt: 100,
      updatedAt: 100,
      lastConnectedAt: null,
    });
    expect(displayHost('ws://10.0.0.4:34123/v1')).toBe('10.0.0.4:34123');
  });

  test('keeps an imported transport kind, including when editing', () => {
    const imported = normalizeDaemonProfile(
      { name: 'Tunnel', address: 'wss://abc.trycloudflare.com', kind: 'cloudflare' },
      undefined,
      'daemon-id',
      100,
    );
    expect(imported.kind).toBe('cloudflare');

    // Renaming an imported profile must not drop its transport.
    const edited = normalizeDaemonProfile(
      { name: 'Renamed', address: 'wss://abc.trycloudflare.com' },
      imported,
      'daemon-id',
      200,
    );
    expect(edited.kind).toBe('cloudflare');
    expect(edited.createdAt).toBe(100);
  });

  test('identifies LAN and tailnet addresses', () => {
    expect(isPrivateDaemonAddress('ws://192.168.1.8:34123')).toBe(true);
    expect(isPrivateDaemonAddress('ws://100.100.12.8:34123')).toBe(true);
    expect(isPrivateDaemonAddress('ws://workstation:34123')).toBe(true);
    expect(isPrivateDaemonAddress('ws://[::1]:34123')).toBe(true);
    expect(isPrivateDaemonAddress('ws://[2001:db8::8]:34123')).toBe(false);
    expect(isPrivateDaemonAddress('wss://padu.example.com')).toBe(false);
  });

  test('creates compact initials', () => {
    expect(profileInitials('Home Mac')).toBe('HM');
    expect(profileInitials('studio')).toBe('ST');
  });

  test('recovers valid profiles from a partially corrupt registry', () => {
    expect(parseDaemonProfiles([
      {
        id: 'one',
        name: 'Home',
        address: 'home.local:34123',
        createdAt: 1,
        updatedAt: 2,
        lastConnectedAt: null,
      },
      { id: 'broken' },
    ])).toEqual([
      {
        id: 'one',
        name: 'Home',
        address: 'ws://home.local:34123',
        kind: 'direct',
        createdAt: 1,
        updatedAt: 2,
        lastConnectedAt: null,
      },
    ]);
  });

  test('preserves a stored transport and degrades an unknown one', () => {
    const profiles = parseDaemonProfiles([
      {
        id: 'tunnel',
        name: 'Tunnel',
        address: 'wss://abc.trycloudflare.com',
        kind: 'cloudflare',
        createdAt: 1,
        updatedAt: 1,
        lastConnectedAt: null,
      },
      {
        id: 'future',
        name: 'Future',
        address: 'wss://future.example.com',
        kind: 'quantum-entanglement',
        createdAt: 1,
        updatedAt: 1,
        lastConnectedAt: null,
      },
    ]);
    expect(profiles.map((profile) => profile.kind)).toEqual([
      'cloudflare',
      'direct',
    ]);
  });

  test('classifies connections from the stored transport', () => {
    expect(classifyConnection('wss://abc.trycloudflare.com', 'cloudflare')).toBe(
      'cloudflare',
    );
    expect(classifyConnection('wss://jump.example.com:19999', 'ssh_relay')).toBe(
      'ssh_relay',
    );
    expect(classifyConnection('wss://mac.tail-abc.ts.net', 'tailscale')).toBe(
      'tailscale',
    );
  });

  test('classifies connections from the address when no transport is stored', () => {
    expect(classifyConnection('wss://abc.trycloudflare.com')).toBe('cloudflare');
    expect(classifyConnection('wss://mac.tail-abc.ts.net')).toBe('tailscale');
    expect(classifyConnection('wss://100.100.50.5:34123')).toBe('tailscale');
    expect(classifyConnection('wss://app.example.com')).toBe('public_wss');
    expect(classifyConnection('ws://10.0.0.5:34123')).toBe('private_ws');
    expect(classifyConnection('ws://app.example.com')).toBe('insecure_public_ws');
    expect(classifyConnection('ftp://nope')).toBe('invalid');
  });
});
