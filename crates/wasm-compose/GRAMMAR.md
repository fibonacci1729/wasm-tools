# Grammar
```
        document ::= {item} .
            item ::= import | statement | export .
          import ::= interface-import | component-import .
interface-import ::= `import` kebab-name `:` id .
component-import ::= `use` package-name [`as` kebab-name] .
          export ::= `export` path [`as` name-or-id] .
       statement ::= `let` kebab-name `=` expression .
      expression ::= instantiate-expr | alias . 
instantiate-expr ::= `new` kebab-name [arguments] .
       arguments ::= `{` argument {`,` argument} [`,`] `}` .
        argument ::= [kebab-name `=`] expression .
           alias ::= path
            path ::= kebab-name {`[` name-or-id `]`} .
      name-or-id ::= kebab-name | id .
              id ::= /* from CM, e.g: foo:bar/baz@0.1.0 */ .
      kebab-name ::= /* from CM, e.g: foo-bar */ .
    package-name ::= /* package subset of id, e.g: foo:bar@0.1.0 */ .
```

## Keywords
```
import export use let new as
```