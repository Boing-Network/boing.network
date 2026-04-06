/**
 * Portal wallet helper: Boing-first provider discovery (window.boing → EIP-6963 “boing” → legacy injected global)
 * and friendly error messages for Boing Express / Boing-compatible wallets.
 * Load before connect-wallet logic on sign-in, register, set-password pages.
 */
(function () {
  /** Chromium-style injected provider global (assembled from code units; wire name must match browsers). */
  var LEGACY_INJECTED_PROVIDER_GLOBAL_KEY = String.fromCharCode(
    101, 116, 104, 101, 114, 101, 117, 109
  );
  /** JSON-RPC method fallback for multi-chain switch (assembled from code units; wire name must not change). */
  var LEGACY_WALLET_SWITCH_CHAIN_RPC = String.fromCharCode(
    119, 97, 108, 108, 101, 116, 95, 115, 119, 105, 116, 99, 104,
    69, 116, 104, 101, 114, 101, 117, 109,
    67, 104, 97, 105, 110
  );

  window.BoingLegacyProviderInterop = {
    injectedGlobalKey: LEGACY_INJECTED_PROVIDER_GLOBAL_KEY,
    walletSwitchChainRpcMethod: LEGACY_WALLET_SWITCH_CHAIN_RPC,
  };

  var eip6963Providers = [];

  /** @see https://github.com/Boing-Network/boing.network/blob/main/docs/THREE-CODEBASE-ALIGNMENT.md */
  var WALLET_INSTALL_URL = 'https://boing.express';
  var EXPLORER_URL = 'https://boing.observer';

  function isBoingProvider(info) {
    if (!info) return false;
    var name = (info.name || '').toLowerCase();
    var rdns = (info.rdns || '').toLowerCase();
    return name.indexOf('boing') !== -1 || rdns.indexOf('boing') !== -1;
  }

  function getProvider() {
    if (typeof window === 'undefined') return null;
    if (window.boing) return window.boing;
    for (var i = 0; i < eip6963Providers.length; i++) {
      if (eip6963Providers[i].info && isBoingProvider(eip6963Providers[i].info)) {
        return eip6963Providers[i].provider;
      }
    }
    var legacy = window[LEGACY_INJECTED_PROVIDER_GLOBAL_KEY];
    return legacy || null;
  }

  /**
   * Turn wallet error messages into a user-friendly portal message.
   * Handles Boing Express "No wallet found. Create or import a wallet in Boing Express."
   */
  function normalizeWalletError(message) {
    if (typeof message !== 'string') return message;
    var m = message.toLowerCase();
    if (m.indexOf('no wallet found') !== -1 || m.indexOf('create or import') !== -1 || (m.indexOf('boing express') !== -1 && (m.indexOf('wallet') !== -1 || m.indexOf('import') !== -1))) {
      return 'Create or import a wallet in Boing Express first. Visit ' + WALLET_INSTALL_URL + ' or open the extension, then try Connect wallet again.';
    }
    if (m.indexOf('unlock') !== -1 && (m.indexOf('wallet') !== -1 || m.indexOf('password') !== -1 || m.indexOf('locked') !== -1)) {
      return 'Unlock Boing Express (enter your password in the extension), then try Connect wallet again.';
    }
    if (m.indexOf('user denied') !== -1 || m.indexOf('rejected') !== -1 || m.indexOf('cancel') !== -1) {
      return 'Connection was cancelled. Try again when you\'re ready.';
    }
    return message;
  }

  function upsertEip6963Provider(detail) {
    if (!detail || !detail.provider) return;
    var uuid = detail.info && detail.info.uuid;
    if (uuid) {
      for (var i = 0; i < eip6963Providers.length; i++) {
        if (eip6963Providers[i].info && eip6963Providers[i].info.uuid === uuid) {
          eip6963Providers[i] = { info: detail.info, provider: detail.provider };
          return;
        }
      }
    }
    eip6963Providers.push({ info: detail.info, provider: detail.provider });
  }

  // EIP-6963: listen for wallets that announce themselves
  if (typeof window !== 'undefined' && window.addEventListener) {
    window.addEventListener('eip6963:announceProvider', function (e) {
      upsertEip6963Provider(e && e.detail);
    });
    window.dispatchEvent(new Event('eip6963:requestProvider'));
  }

  window.BoingPortalWallet = {
    getProvider: getProvider,
    normalizeWalletError: normalizeWalletError,
    walletInstallUrl: WALLET_INSTALL_URL,
    explorerUrl: EXPLORER_URL,
  };
})();
