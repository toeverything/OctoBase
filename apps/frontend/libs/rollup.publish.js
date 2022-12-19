const nrwlConfig = require('@nrwl/react/plugins/bundle-rollup');
const replace = require('@rollup/plugin-replace').default;

module.exports = config => {
    const nxConfig = nrwlConfig(config);

    return {
        ...nxConfig,
        plugins: [
            ...nxConfig.plugins,
            replace({
                JWT_DEV: false,
            }),
        ],
    };
};
