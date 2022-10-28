const { merge } = require('webpack-merge');

module.exports = (config, context) => {
    return merge(config, {
        output: {
            publicPath: '',
        },
    });
};
