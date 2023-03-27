# How to organize data with OctoBase

In OctoBase, we unify different data structures into the concept of Block, and different Blocks have similar properties.

For example, headings, normal text lines, and Todo all have a text content property in common, but their flavour is different, while Todo has a clicked property that confirms completion.

In this way, we can define different Block flavours to represent different data types, for example:

```js
const titleBlock = {
	'sys:flavour': 'affine:title',
	'sys:created': 1666158236651,
	'sys:children': [],
	'prop:text': 'This is a Title',
}
const textBlock = {
	'sys:flavour': 'affine:text',
	'sys:created': 1666158236651,
	'sys:children': [],
	'prop:text': 'This is a normal line',
}
const todoBlock = {
	'sys:flavour': 'affine:todo',
	'sys:created': 1666158236651,
	'sys:children': [],
	'prop:text': 'This is a todo',
	'prop:clicked': false,
}
```

To illustrate with a simple example, suppose we have a page with a title, a todo list, and a normal text line, as shown in the following image:

![block structure to view](./assets/how_to_organize_your_data_1.jpg)

In OctoBase, we can define it like this:

1. We treat a page as a Block
2. We also treat the title and text line as a Block
3. Each line of content in a Page is treated as a child in the Page Block

Then we can reorganize the data shown in the figure above with Block:

```js
const title = {
	'sys:flavour': 'affine:title',
	'sys:created': 1666158236651,
	'sys:children': [],
	'prop:text': 'Welcome to the AFFiNE Alpha',
}
const text = {
	'sys:flavour': 'affine:text',
	'sys:created': 1666158236651,
	'sys:children': [],
	// Here we ignore how to express rich text Link
	'prop:text': 'The AFFiNE Alpha is here! You can also view our Official Website!',
}
const todo1 = {
	'sys:flavour': 'affine:todo',
	'sys:created': 1666158236651,
	'sys:children': [],
	'prop:text': 'Try AFFiNE Alpha',
	'prop:clicked': true,
}
const todo2 = {
	'sys:flavour': 'affine:todo',
	'sys:created': 1666158236651,
	'sys:children': [],
	'prop:text': 'Have a good night',
	'prop:clicked': false,
}

title.children = [text, todo1, todo2]
```

At this point we have reorganized a rich text page into structured data, and now we can:

-   Change the order of text lines by adjusting the order of children
-   Change the text line style by adjusting the flavour
-   Change the actual text content by adjusting the content in the props

In actual use, you do not need to manually edit the data in the structure. OctoBase provides a series of easy-to-use APIs that allow you:

-   Organize parent-child relationship of blocks, front and back order, etc.
-   Modify the properties with basic data structures such as RichText, Map, Array, String, Number, etc.
-   Reactively update data when local or remote modifications occur

And all these modifications can be Conflict-free merge with any remote offline.
