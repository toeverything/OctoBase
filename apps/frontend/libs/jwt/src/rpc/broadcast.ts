import * as bc from 'lib0/broadcastchannel';
import * as encoding from 'lib0/encoding';
import { Observable } from 'lib0/observable';
import * as awarenessProtocol from 'y-protocols/awareness';
import * as syncProtocol from 'y-protocols/sync';
import * as Y from 'yjs';
import { handler, Message } from './handler';
import { readMessage } from './processor';

// @todo - this should depend on awareness.outdatedTime
const messageReconnectTimeout = 30000;

export class BroadcastProvider extends Observable<string> {
    doc: Y.Doc;
    awareness: awarenessProtocol.Awareness;
    messageHandlers: typeof handler;
    shouldConnect: boolean;
    ws: any;
    wsconnecting: boolean;
    wsconnected: boolean;
    wsLastMessageReceived: number;
    wsUnsuccessfulReconnects: any;
    maxBackOffTime: number;
    bcconnected: any;
    bcChannel: string;
    roomName: string;
    _synced: boolean;
    _resyncInterval: any;
    extraToleranceTime: number;
    _bcSubscriber: (data: ArrayBuffer, origin: any) => void;
    _updateHandler: (update: Uint8Array, origin: any) => void;
    _awarenessUpdateHandler: ({ added, updated, removed }: any) => void;
    _unloadHandler: () => void;
    _checkInterval: NodeJS.Timer;

    constructor(
        serverUrl: string,
        roomName: string,
        doc: Y.Doc,
        {
            connect = true,
            awareness = new awarenessProtocol.Awareness(doc),

            resyncInterval = -1,
            maxBackOffTime = 2500,
            extraToleranceTime = 0,
        } = {}
    ) {
        super();

        this.maxBackOffTime = maxBackOffTime;
        this.extraToleranceTime = extraToleranceTime;
        this.bcChannel = serverUrl + '/' + roomName;

        this.roomName = roomName;
        this.doc = doc;
        this.awareness = awareness;
        this.wsconnected = false;
        this.wsconnecting = false;
        this.bcconnected = false;
        this.wsUnsuccessfulReconnects = 0;
        this.messageHandlers = handler;

        this._synced = false;
        /**
         * @type {WebSocket?}
         */
        this.ws = null;
        this.wsLastMessageReceived = 0;

        this.shouldConnect = connect;

        this._resyncInterval = 0;
        if (resyncInterval > 0) {
            this._resyncInterval = /** @type {any} */ setInterval(() => {
                if (this.ws && this.ws.readyState === WebSocket.OPEN) {
                    // resend sync step 1
                    const encoder = encoding.createEncoder();
                    encoding.writeVarUint(encoder, Message.sync);
                    syncProtocol.writeSyncStep1(encoder, doc);
                    this.ws.send(encoding.toUint8Array(encoder));
                }
            }, resyncInterval);
        }

        this._bcSubscriber = (data: ArrayBuffer, origin: any) => {
            if (origin !== this) {
                const encoder = readMessage(this, new Uint8Array(data), false);
                if (encoding.length(encoder) > 1) {
                    bc.publish(
                        this.bcChannel,
                        encoding.toUint8Array(encoder),
                        this
                    );
                }
            }
        };

        this._updateHandler = (update: Uint8Array, origin: any) => {
            if (origin !== this) {
                const encoder = encoding.createEncoder();
                encoding.writeVarUint(encoder, Message.sync);
                syncProtocol.writeUpdate(encoder, update);

                if (this.bcconnected) {
                    bc.publish(
                        this.bcChannel,
                        encoding.toUint8Array(encoder),
                        this
                    );
                }
            }
        };
        this.doc.on('update', this._updateHandler);

        this._awarenessUpdateHandler = ({ added, updated, removed }: any) => {
            const changedClients = added.concat(updated).concat(removed);
            const encoder = encoding.createEncoder();
            encoding.writeVarUint(encoder, Message.awareness);
            encoding.writeVarUint8Array(
                encoder,
                awarenessProtocol.encodeAwarenessUpdate(
                    awareness,
                    changedClients
                )
            );

            if (this.bcconnected) {
                bc.publish(
                    this.bcChannel,
                    encoding.toUint8Array(encoder),
                    this
                );
            }
        };
        this._unloadHandler = () => {
            awarenessProtocol.removeAwarenessStates(
                this.awareness,
                [doc.clientID],
                'window unload'
            );
        };
        if (typeof window !== 'undefined') {
            window.addEventListener('unload', this._unloadHandler);
        } else if (typeof process !== 'undefined') {
            process.on('exit', this._unloadHandler);
        }
        awareness.on('update', this._awarenessUpdateHandler);
        this._checkInterval = /** @type {any} */ setInterval(() => {
            if (
                this.wsconnected &&
                messageReconnectTimeout <
                    Date.now() - this.wsLastMessageReceived
            ) {
                // no message received in a long time - not even your own awareness
                // updates (which are updated every 15 seconds)
                /** @type {WebSocket} */ this.ws.close();
            }
        }, messageReconnectTimeout / 10);
        if (connect) {
            this.connect();
        }
    }

    get synced() {
        return this._synced;
    }

    set synced(state) {
        if (this._synced !== state) {
            this._synced = state;
            this.emit('synced', [state]);
            this.emit('sync', [state]);
        }
    }

    override destroy() {
        if (this._resyncInterval !== 0) {
            clearInterval(this._resyncInterval);
        }
        clearInterval(this._checkInterval);
        this.disconnect();
        if (typeof window !== 'undefined') {
            window.removeEventListener('unload', this._unloadHandler);
        } else if (typeof process !== 'undefined') {
            process.off('exit', this._unloadHandler);
        }
        this.awareness.off('update', this._awarenessUpdateHandler);
        this.doc.off('update', this._updateHandler);
        super.destroy();
    }

    disconnect() {
        if (this.bcconnected) {
            // broadcast message with local awareness state set to null (indicating disconnect)
            const encoder = encoding.createEncoder();
            encoding.writeVarUint(encoder, Message.awareness);
            encoding.writeVarUint8Array(
                encoder,
                awarenessProtocol.encodeAwarenessUpdate(
                    this.awareness,
                    [this.doc.clientID],
                    new Map()
                )
            );
            bc.publish(this.bcChannel, encoding.toUint8Array(encoder), this);

            bc.unsubscribe(this.bcChannel, this._bcSubscriber);
            this.bcconnected = false;
        }
    }

    connect() {
        if (!this.bcconnected) {
            bc.subscribe(this.bcChannel, this._bcSubscriber);
            this.bcconnected = true;
        }
        // send sync step1 to bc
        // write sync step 1
        const encoderSync = encoding.createEncoder();
        encoding.writeVarUint(encoderSync, Message.sync);
        syncProtocol.writeSyncStep1(encoderSync, this.doc);
        bc.publish(this.bcChannel, encoding.toUint8Array(encoderSync), this);
        // broadcast local state
        const encoderState = encoding.createEncoder();
        encoding.writeVarUint(encoderState, Message.sync);
        syncProtocol.writeSyncStep2(encoderState, this.doc);
        bc.publish(this.bcChannel, encoding.toUint8Array(encoderState), this);
        // write queryAwareness
        const encoderAwarenessQuery = encoding.createEncoder();
        encoding.writeVarUint(encoderAwarenessQuery, Message.queryAwareness);
        bc.publish(
            this.bcChannel,
            encoding.toUint8Array(encoderAwarenessQuery),
            this
        );
        // broadcast local awareness state
        const encoderAwarenessState = encoding.createEncoder();
        encoding.writeVarUint(encoderAwarenessState, Message.awareness);
        encoding.writeVarUint8Array(
            encoderAwarenessState,
            awarenessProtocol.encodeAwarenessUpdate(this.awareness, [
                this.doc.clientID,
            ])
        );
        bc.publish(
            this.bcChannel,
            encoding.toUint8Array(encoderAwarenessState),
            this
        );
    }
}
