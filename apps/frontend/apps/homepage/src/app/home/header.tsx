import { Grid, Typography } from '@mui/material';

import { AlignCenteredContainer, ShadowBox } from './common';

export const Header = () => (
    <ShadowBox>
        <AlignCenteredContainer maxWidth="md" sx={{ height: '3rem' }}>
            <Grid container>
                <Grid item xs={2}>
                    <Typography variant="h5">JWSTBase</Typography>
                </Grid>
            </Grid>
        </AlignCenteredContainer>
    </ShadowBox>
);
