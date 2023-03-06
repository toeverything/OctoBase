export type BlockContent = {
    id?: string;
    flavor: string;
    parentId: string;
    properties: {
        [prop: string]: unknown;
    };
};

export type BinaryContent = {
    binary?: ArrayBuffer;
    parentId: string;
    properties: {
        [prop: string]: unknown;
    };
};

export type UpdateBlock = {
    id: string;
    content: Partial<BlockContent>;
};

export type InsertOperation = {
    type: 'InsertBlockOperation';
    content: Partial<BlockContent>;
};

export type InsertBinaryOperation = {
    type: 'InsertBinaryOperation';
    content: Partial<BinaryContent>;
};

export type UpdateOperation = {
    type: 'UpdateBlockOperation';
    id: string;
    content: Partial<BlockContent>;
};

export type DeleteOperation = {
    type: 'DeleteBlockOperation';
    id: string;
};

export type Operation =
    | InsertOperation
    | InsertBinaryOperation
    | UpdateOperation
    | DeleteOperation;
