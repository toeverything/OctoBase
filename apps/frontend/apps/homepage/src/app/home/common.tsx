import { Box, Container, styled } from '@mui/material';

export const AlignCenteredContainer = styled(Container)({
    display: 'flex',
    alignItems: 'center',
});

export const Link = styled('a')({
    fontWeight: '900',
    color: '#000',
    textDecoration: 'none',

    '&:hover': {
        textDecoration: 'underline',
    },
});

export const ShadowBox = styled(Box)({
    boxShadow: '0px 0px 10px 0px rgba(0,0,0,0.1)',
});
