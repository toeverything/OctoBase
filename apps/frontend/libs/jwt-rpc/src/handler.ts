import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import * as syncProtocol from 'y-protocols/sync';

import { KeckProvider } from './keckprovider';

export enum Message {
    sync = 0,
}

export type MessageCallback = (
    encoder: encoding.Encoder,
    decoder: decoding.Decoder,
    provider: KeckProvider | KeckProvider,
    emitSynced: boolean,
    messageType: number
) => void;

export const handler: Record<Message, MessageCallback> = {
    [Message.sync]: (encoder, decoder, provider, emitSynced, messageType) => {
        encoding.writeVarUint(encoder, Message.sync);
        const syncMessageType = syncProtocol.readSyncMessage(
            decoder,
            encoder,
            provider.doc,
            provider
        );
        if (
            emitSynced &&
            syncMessageType === syncProtocol.messageYjsSyncStep2 &&
            !provider.synced
        ) {
            provider.synced = true;
        }
    },
};
