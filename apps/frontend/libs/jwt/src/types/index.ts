export { BlockFlavors } from './block';
export type { BlockItem } from './block';
export type { ExcludeFunction } from './utils';

function getLocation() {
    try {
        const { protocol, host } = window.location;
        return { protocol, host };
    } catch (e) {
        return { protocol: 'http:', host: 'localhost' };
    }
}

function getCollaborationPoint() {
    const { protocol, host } = getLocation();
    const isOnline = protocol.startsWith('https');
    const ws = isOnline ? 'wss' : 'ws';
    const site = isOnline ? host : 'localhost:3000';
    return `${ws}://${site}/collaboration/`;
}

export const BucketBackend = {
    IndexedDB: 'idb',
    WebSQL: 'websql',
    YWebSocketAffine: getCollaborationPoint(),
};
