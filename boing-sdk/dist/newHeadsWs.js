/**
 * Browser-oriented **`GET /ws`** **newHeads** subscriber (handshake + JSON text frames).
 * Uses global **`WebSocket`**; safe to omit in SSR bundles if you never instantiate.
 */
/** Build **`ws:` / `wss:`** URL for the node’s WebSocket endpoint. */
export function boingRpcWebSocketUrl(rpcBaseUrl, path = '/ws') {
    const trimmed = rpcBaseUrl.replace(/\/$/, '');
    const withProto = trimmed.includes('://') ? trimmed : `http://${trimmed}`;
    const u = new URL(withProto);
    u.protocol = u.protocol === 'https:' ? 'wss:' : 'ws:';
    const p = path.startsWith('/') ? path : `/${path}`;
    u.pathname = p;
    u.search = '';
    u.hash = '';
    return u.toString();
}
const SUBSCRIBE = JSON.stringify({ type: 'subscribe', channel: 'newHeads' });
/**
 * Thin wrapper around **`WebSocket`** with reconnect and typed callbacks.
 */
export class BoingNewHeadsWs {
    constructor(rpcBaseUrl, options = {}) {
        this.rpcBaseUrl = rpcBaseUrl;
        this.options = options;
        this.ws = null;
        this.state = 'idle';
        this.reconnectAttempt = 0;
        this.reconnectTimer = null;
        this.closedByUser = false;
    }
    setState(s) {
        this.state = s;
        this.options.onState?.(s);
    }
    getState() {
        return this.state;
    }
    /** Open socket and send subscribe handshake. */
    connect() {
        const WS = globalThis.WebSocket;
        if (WS == null) {
            const err = new Error('WebSocket is not available in this environment');
            this.setState('error');
            this.options.onError?.(err);
            throw err;
        }
        this.closedByUser = false;
        if (this.reconnectTimer != null) {
            clearTimeout(this.reconnectTimer);
            this.reconnectTimer = null;
        }
        const path = this.options.path ?? '/ws';
        const url = boingRpcWebSocketUrl(this.rpcBaseUrl, path);
        this.setState(this.reconnectAttempt > 0 ? 'reconnecting' : 'connecting');
        const ws = new WS(url);
        this.ws = ws;
        ws.onopen = () => {
            try {
                ws.send(SUBSCRIBE);
            }
            catch (e) {
                this.options.onError?.(e);
            }
        };
        ws.onmessage = (ev) => {
            if (typeof ev.data !== 'string')
                return;
            let j;
            try {
                j = JSON.parse(ev.data);
            }
            catch {
                return;
            }
            if (!j || typeof j !== 'object')
                return;
            const o = j;
            if (o.type === 'subscribed' && o.channel === 'newHeads') {
                this.reconnectAttempt = 0;
                this.setState('subscribed');
                this.options.onSubscribed?.();
                return;
            }
            if (o.type === 'newHead') {
                this.options.onHead?.(j);
                return;
            }
            if (o.type === 'error') {
                this.options.onError?.(j);
            }
        };
        ws.onerror = () => {
            this.options.onError?.(new Error('WebSocket error'));
        };
        ws.onclose = () => {
            this.ws = null;
            if (this.closedByUser) {
                this.setState('closed');
                return;
            }
            this.scheduleReconnect();
        };
    }
    scheduleReconnect() {
        const maxAttempts = this.options.maxReconnectAttempts ?? 8;
        if (this.reconnectAttempt >= maxAttempts) {
            this.setState('error');
            this.options.onError?.(new Error('WebSocket: max reconnect attempts reached'));
            return;
        }
        const base = this.options.initialReconnectDelayMs ?? 1000;
        const maxDelay = this.options.maxReconnectDelayMs ?? 30000;
        const delay = Math.min(maxDelay, base * Math.pow(2, this.reconnectAttempt));
        this.reconnectAttempt += 1;
        this.setState('reconnecting');
        this.reconnectTimer = setTimeout(() => {
            this.reconnectTimer = null;
            this.connect();
        }, delay);
    }
    /** Stop reconnecting and close the socket. */
    close() {
        this.closedByUser = true;
        if (this.reconnectTimer != null) {
            clearTimeout(this.reconnectTimer);
            this.reconnectTimer = null;
        }
        if (this.ws != null) {
            try {
                this.ws.close();
            }
            catch {
                /* ignore */
            }
            this.ws = null;
        }
        this.setState('closed');
    }
}
