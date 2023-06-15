import w from 'nextra';
import * as dotenv from 'dotenv';

const env = dotenv.config().parsed || {};
const DEBUG_LOCAL = process.env.DEBUG_LOCAL || env.DEBUG_LOCAL;
const IS_DEBUG_LOCAL = !!DEBUG_LOCAL && DEBUG_LOCAL.toLocaleLowerCase() === 'true'
const PORT = process.env.PORT || env.PORT;

const withNextra = w(
    {
        theme: 'nextra-theme-docs',
        themeConfig: './theme.config.jsx',
    }
);

const profileTarget = {
    prod: 'https://app.affine.pro', // TODO need target to public keck server
    local: 'http://127.0.0.1',
};

const target = IS_DEBUG_LOCAL ? profileTarget.local : profileTarget.prod;
const targetUrl = new URL(target);
const wsProtocol = targetUrl.protocol === 'https:' ? 'wss' : 'ws';
const wsPort = PORT || 3001;

export default withNextra({
    images: {
        unoptimized: true,
    },
    publicRuntimeConfig: {
        websocketPrefixUrl: `${wsProtocol}://${targetUrl.host}:${wsPort}`,
    },
});
