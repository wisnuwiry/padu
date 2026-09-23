import { describe, expect, test } from 'bun:test';

import {
  buildConnectUrl,
  connectLinkFallbackName,
  encodeBase64Url,
  parseConnectUrl,
  type ConnectLink,
} from './daemon-connect-link';

describe('daemon connect links', () => {
  test('parses a cloudflare link produced by the desktop encoder', () => {
    // This is the exact shape `url::Url`'s form serializer emits: the token is
    // base64url and a space in the name arrives as `+`.
    const parsed = parseConnectUrl(
      'padu://connect?v=1&kind=cloudflare&url=wss%3A%2F%2Fxyz.trycloudflare.com&token=YWJjMTIz&name=My+Mac',
    );
    expect(parsed).toEqual({
      kind: 'cloudflare',
      address: 'wss://xyz.trycloudflare.com',
      token: 'abc123',
      name: 'My Mac',
    });
  });

  test('round-trips every transport kind', () => {
    const kinds: ConnectLink['kind'][] = [
      'direct',
      'tailscale',
      'cloudflare',
      'ssh_relay',
    ];
    for (const kind of kinds) {
      const link: ConnectLink = {
        kind,
        address: 'wss://daemon.example.com:34123',
        token: 'secret-token-value',
        name: 'Studio',
      };
      expect(parseConnectUrl(buildConnectUrl(link))).toEqual(link);
    }
  });

  test('round-trips a non-ASCII name and a token with padding characters', () => {
    const link: ConnectLink = {
      kind: 'ssh_relay',
      address: 'wss://jump.example.com:19999',
      // Real tokens are `Uuid::new_v4().simple()`, but padding and `+`/`/`
      // would be the interesting failure modes for a naive encoder.
      token: 'a+b/c==',
      name: 'My MacBook (dev) 🌱',
    };
    expect(parseConnectUrl(buildConnectUrl(link))).toEqual(link);
  });

  test('builds a base64url token without padding', () => {
    expect(encodeBase64Url('abc')).toBe('YWJj');
    expect(encodeBase64Url('ab')).toBe('YWI');
    expect(encodeBase64Url('a')).toBe('YQ');
    expect(encodeBase64Url('')).toBe('');
  });

  test('rejects links that are not connect links', () => {
    expect(parseConnectUrl('')).toBeNull();
    expect(parseConnectUrl('https://example.com/?v=1')).toBeNull();
    expect(parseConnectUrl('padu://other?v=1')).toBeNull();
    expect(parseConnectUrl('not a url at all')).toBeNull();
  });

  test('rejects unsupported versions and unknown transports', () => {
    expect(
      parseConnectUrl(
        'padu://connect?v=2&kind=direct&url=wss%3A%2F%2Fx.com&token=YQ&name=x',
      ),
    ).toBeNull();
    expect(
      parseConnectUrl(
        'padu://connect?v=1&kind=carrier-pigeon&url=wss%3A%2F%2Fx.com&token=YQ&name=x',
      ),
    ).toBeNull();
  });

  test('rejects links missing required fields', () => {
    const required = ['v=1', 'kind=direct', 'url=wss%3A%2F%2Fx.com', 'token=YQ'];
    for (let drop = 0; drop < required.length; drop += 1) {
      const query = required.filter((_, i) => i !== drop).join('&');
      expect(parseConnectUrl(`padu://connect?${query}`)).toBeNull();
    }
  });

  test('rejects a non-websocket address', () => {
    expect(
      parseConnectUrl(
        'padu://connect?v=1&kind=direct&url=https%3A%2F%2Fx.com&token=YQ&name=x',
      ),
    ).toBeNull();
  });

  test('rejects a token that is not valid base64url', () => {
    expect(
      parseConnectUrl(
        'padu://connect?v=1&kind=direct&url=wss%3A%2F%2Fx.com&token=not*valid&name=x',
      ),
    ).toBeNull();
  });

  test('falls back to the host when the link carries no name', () => {
    expect(
      connectLinkFallbackName({
        kind: 'direct',
        address: 'ws://10.0.0.4:34123',
        token: 't',
        name: '   ',
      }),
    ).toBe('10.0.0.4:34123');
    expect(
      connectLinkFallbackName({
        kind: 'direct',
        address: 'wss://work.example.com',
        token: 't',
        name: 'Work',
      }),
    ).toBe('Work');
  });
});
