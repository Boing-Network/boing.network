/**
 * Testnet portal session (localStorage). Load on portal pages via <script src="/portal-session.js">.
 * Usage: BoingPortalSession.getSession() | .setSession(accountIdHex, role, tracks?) | .hasTrack(session, track) | .clearSession()
 * Testnet Passport: an account can hold several tracks (user | developer | node_operator); `role` is the primary one.
 */
(function () {
  var key = 'boing_portal_session';
  function getSession() {
    try {
      var s = localStorage.getItem(key);
      return s ? JSON.parse(s) : null;
    } catch (e) {
      return null;
    }
  }
  function setSession(accountIdHex, role, tracks) {
    try {
      var prev = getSession();
      var merged = [];
      function add(t) { if (t && merged.indexOf(t) === -1) merged.push(t); }
      if (prev && prev.account_id_hex && accountIdHex && String(prev.account_id_hex).toLowerCase() === String(accountIdHex).toLowerCase()) {
        (prev.tracks || []).forEach(add);
        add(prev.role);
      }
      (tracks || []).forEach(add);
      add(role);
      localStorage.setItem(key, JSON.stringify({ account_id_hex: accountIdHex, role: role, tracks: merged }));
      return true;
    } catch (e) {
      return false;
    }
  }
  function hasTrack(session, track) {
    if (!session) return false;
    if (session.role === track) return true;
    return Array.isArray(session.tracks) && session.tracks.indexOf(track) !== -1;
  }
  function clearSession() {
    try {
      localStorage.removeItem(key);
    } catch (e) {}
  }
  /** Redirect to main Boing Network website and clear session (logout). */
  function logout() {
    clearSession();
    var main = typeof BOING_MAIN_SITE_URL !== 'undefined' ? BOING_MAIN_SITE_URL : '/';
    window.location.href = main;
  }
  function shortAccount(hex) {
    if (!hex || hex.length < 14) return hex || '';
    return hex.slice(0, 8) + '…' + hex.slice(-6);
  }
  window.BoingPortalSession = {
    getSession: getSession,
    setSession: setSession,
    hasTrack: hasTrack,
    clearSession: clearSession,
    logout: logout,
    shortAccount: shortAccount,
    roleAppPath: function (role) {
      if (role === 'developer') return '/testnet/developers';
      if (role === 'user') return '/testnet/users';
      if (role === 'node_operator') return '/testnet/operators';
      return '/testnet';
    }
  };
})();
