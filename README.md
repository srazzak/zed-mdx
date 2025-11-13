# Zed MDX

A MDX extension for Zed.

## Typescript configuration

This extension provides LSP support through [mdx-analyzer](https://github.com/mdx-js/mdx-analyzer). As part of that, Typescript support is provided by [@mdx-js/typescript-plugin](https://github.com/mdx-js/mdx-analyzer/tree/main/packages/typescript-plugin). 

However, since mdx-analyzer defaults to disabling plugin, this extension also defaults to that; meaning Typescript support is disabled by default. To enable Typescript support, you'll need to override the LSP settings in Zed's `settings.json`. 

You can use the snippet below to enable Typescript support for mdx-analyzer.

```json
"lsp": {
    "mdx-analyzer": {
        "initialization_options": {
            "typescript": {
                "enabled": true
            }
        }
    }
}
```
