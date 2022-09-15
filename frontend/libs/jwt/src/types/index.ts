export { BlockFlavors, BlockTypes } from './block';
export type { BlockItem, BlockTypeKeys } from './block';
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
    const ws = protocol.startsWith('https') ? 'wss' : 'ws';
    const isOnline = host.endsWith('affine.pro');
    const site = isOnline ? host : 'localhost:3000';
    return `${ws}://${site}/collaboration/`;
}

export const BucketBackend = {
    IndexedDB: 'idb',
    WebSQL: 'websql',
    YWebSocketAffine: getCollaborationPoint(),
};
