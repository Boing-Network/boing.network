/**
 * Portal wallet helper: provider discovery (window.boing, EIP-6963, window.ethereum)
 * and friendly error messages for Boing Express / Boing-compatible wallets.
 * Load before connect-wallet logic on sign-in, register, set-password pages.
 */
(function () {
  var eip6963Providers = [];

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
    return window.ethereum || null;
  }

  /**
   * Turn wallet error messages into a user-friendly portal message.
   * Handles Boing Express "No wallet found. Create or import a wallet in Boing Express."
   */
  function normalizeWalletError(message) {
    if (typeof message !== 'string') return message;
    var m = message.toLowerCase();
    if (m.indexOf('no wallet found') !== -1 || m.indexOf('create or import') !== -1 || (m.indexOf('boing express') !== -1 && (m.indexOf('wallet') !== -1 || m.indexOf('import') !== -1))) {
      return 'Create or import a wallet in Boing Express first. Click the Boing Express extension icon in your browser, create or import a wallet, then try Connect wallet again.';
    }
    if (m.indexOf('user denied') !== -1 || m.indexOf('rejected') !== -1 || m.indexOf('cancel') !== -1) {
      return 'Connection was cancelled. Try again when you\'re ready.';
    }
    return message;
  }

  // EIP-6963: listen for wallets that announce themselves
  if (typeof window !== 'undefined' && window.addEventListener) {
    window.addEventListener('eip6963:announceProvider', function (e) {
      var detail = e && e.detail;
      if (detail && detail.provider) {
        eip6963Providers.push({ info: detail.info, provider: detail.provider });
      }
    });
    window.dispatchEvent(new Event('eip6963:requestProvider'));
  }

  window.BoingPortalWallet = {
    getProvider: getProvider,
    normalizeWalletError: normalizeWalletError
  };
})();
