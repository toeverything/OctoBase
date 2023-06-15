import getConfig from 'next/config';

type PublicRuntimeConfig = {
    websocketPrefixUrl: string;
}

const {publicRuntimeConfig: config} = getConfig() as {
    publicRuntimeConfig: PublicRuntimeConfig
};

export const websocketPrefixUrl = config.websocketPrefixUrl;

