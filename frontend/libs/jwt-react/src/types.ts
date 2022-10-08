import { JwtStore } from '@toeverything/jwt';

export type Block = NonNullable<Awaited<ReturnType<JwtStore['get']>>>;

export type BasicBlockData = string | number | boolean;
