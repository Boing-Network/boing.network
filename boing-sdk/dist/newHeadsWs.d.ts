/**
 * Browser-oriented **`GET /ws`** **newHeads** subscriber (handshake + JSON text frames).
 * Uses global **`WebSocket`**; safe to omit in SSR bundles if you never instantiate.
 */
export type BoingNewHeadsWsState = 'idle' | 'connecting' | 'subscribed' | 'reconnecting' | 'closed' | 'error';
export interface BoingNewHeadsWsOptions {
    /** WebSocket path on the RPC host (default **`/ws`**). */
    path?: string;
    /** First reconnect delay in ms (default **1000**); doubles up to **`maxReconnectDelayMs`**. */
    initialReconnectDelayMs?: number;
    maxReconnectDelayMs?: number;
    /** Cap manual reconnect attempts after close (**default 8**). */
    maxReconnectAttempts?: number;
    onState?: (s: BoingNewHeadsWsState) => void;
    /** Fired for server **`newHead`** payloads (parsed JSON). */
    onHead?: (payload: unknown) => void;
    /** Subscribe handshake succeeded. */
    onSubscribed?: () => void;
    onError?: (err: unknown) => void;
}
/** Build **`ws:` / `wss:`** URL for the node’s WebSocket endpoint. */
export declare function boingRpcWebSocketUrl(rpcBaseUrl: string, path?: string): string;
/**
 * Thin wrapper around **`WebSocket`** with reconnect and typed callbacks.
 */
export declare class BoingNewHeadsWs {
    private readonly rpcBaseUrl;
    private readonly options;
    private ws;
    private state;
    private reconnectAttempt;
    private reconnectTimer;
    private closedByUser;
    constructor(rpcBaseUrl: string, options?: BoingNewHeadsWsOptions);
    private setState;
    getState(): BoingNewHeadsWsState;
    /** Open socket and send subscribe handshake. */
    connect(): void;
    private scheduleReconnect;
    /** Stop reconnecting and close the socket. */
    close(): void;
}
//# sourceMappingURL=newHeadsWs.d.ts.map