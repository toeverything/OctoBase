# How to organize you own data with OctoBase

In OctoBase, we unify different data structures into the concept of Block, and different Blocks have similar properties.

For example, headings, normal text lines, and Todo all have a text content property in common, but their Flavor is different, while Todo has a clicked property that confirms completion.

In this way, we can define different Block Flavors to represent different data types, for example:

```js
const titleBlock = {
	flavor: 'affine:title',
	created: 1666158236651,
	children: [],
	props: {
		text: 'This is a Title',
	},
}
const textBlock = {
	flavor: 'affine:text',
	created: 1666158236651,
	children: [],
	props: {
		text: 'This is a normal line',
	},
}
const todoBlock = {
	flavor: 'affine:todo',
	created: 1666158236651,
	children: [],
	props: {
		text: 'This is a todo',
		clicked: false,
	},
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
	flavor: 'affine:title',
	created: 1666158236651,
	children: [],
	props: {
		text: 'Welcome to the AFFiNE Alpha',
	},
}
const text = {
	flavor: 'affine:text',
	created: 1666158236651,
	children: [],
	props: {
		// Here we ignore how to express rich text Link
		text: 'The AFFiNE Alpha is here! You can also view our Official Website!',
	},
}
const todo1 = {
	flavor: 'affine:todo',
	created: 1666158236651,
	children: [],
	props: {
		text: 'Try AFFiNE Alpha',
		clicked: true,
	},
}
const todo2 = {
	flavor: 'affine:todo',
	created: 1666158236651,
	children: [],
	props: {
		text: 'Have a good night',
		clicked: false,
	},
}

title.children = [text, todo1, todo2]
```

At this point we have reorganized a rich text page into structured data, and now we can:

-   Change the order of text lines by adjusting the order of children
-   Change the text line style by adjusting the flavor
-   Change the actual text content by adjusting the content in the props

In actual use, you do not need to manually edit the data in the structure. OctoBase provides a series of easy-to-use APIs that allow you:

-   Organize parent-child relationship of blocks, front and back order, etc.
-   Modify the properties with basic data structures such as RichText, Map, Array, String, Number, etc.
-   Reactively update data when local or remote modifications occur

And all these modifications can be Conflict-free merge with any remote offline.
