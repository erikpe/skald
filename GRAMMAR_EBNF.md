# MVP Grammar (EBNF, Recursive Descent Friendly)

This grammar matches the MVP language described in MVP_LANGUAGE.md. It is designed to be easy to implement with a handwritten recursive descent parser. It uses a standard precedence ladder for expressions and avoids left recursion.

Notes
- Whitespace and comments are ignored except as token separators.
- Keywords are reserved and cannot be used as identifiers.
- The grammar uses explicit block statements and requires semicolons for simple statements.
- This grammar omits arrays, generics, and modules (MVP scope).


## Tokens

identifier   = letter , { letter | digit | '_' } ;
integer      = digit , { digit } ;

letter       = 'A'..'Z' | 'a'..'z' | '_' ;
digit        = '0'..'9' ;

Keywords
- fn struct var if else while return defer true false null extern

Operators and punctuation
- + - * / % == != < <= > >= && || ! & =
- ( ) { } [ ] ; , : . ->


## Program

program      = { declaration } , EOF ;

declaration  = struct_decl | extern_fn_decl | fn_decl ;


## Declarations

struct_decl  = "struct" , identifier , "{" , { field_decl } , "}" ;

field_decl   = identifier , ":" , type , ";" ;

extern_fn_decl = "extern" , "fn" , identifier , "(" , [ params ] , ")" , "->" , type , ";" ;

fn_decl      = "fn" , identifier , "(" , [ params ] , ")" , "->" , type , block ;

params       = param , { "," , param } ;
param        = identifier , ":" , type ;


## Types

type         = pointer_type | base_type | identifier ;

pointer_type = "*" , type ;

base_type    = "i64" | "u64" | "i32" | "u32" | "bool" | "unit" ;


## Statements

block        = "{" , { statement } , "}" ;

statement    = block
             | var_decl
             | defer_stmt
             | if_stmt
             | while_stmt
             | return_stmt
             | expr_stmt
             ;

var_decl     = "var" , identifier , ":" , type , "=" , expression , ";" ;

defer_stmt   = "defer" , call_expr , ";" ;

if_stmt      = "if" , expression , block , [ "else" , block ] ;

while_stmt   = "while" , expression , block ;

return_stmt  = "return" , [ expression ] , ";" ;

expr_stmt    = expression , ";" ;


## Expressions (Precedence Ladder)

expression   = assignment ;

assignment   = logic_or , [ "=" , assignment ] ;

logic_or     = logic_and , { "||" , logic_and } ;

logic_and    = equality , { "&&" , equality } ;

equality    = relational , { ( "==" | "!=" ) , relational } ;

relational   = additive , { ( "<" | "<=" | ">" | ">=" ) , additive } ;

additive     = multiplicative , { ( "+" | "-" ) , multiplicative } ;

multiplicative = unary , { ( "*" | "/" | "%" ) , unary } ;

unary        = ( "-" | "!" | "*" | "&" ) , unary
             | postfix
             ;

postfix      = primary , { call_suffix | field_suffix } ;

call_suffix  = "(" , [ arguments ] , ")" ;

field_suffix = "." , identifier ;

arguments    = expression , { "," , expression } ;

call_expr    = primary , call_suffix ;

primary      = integer
             | "true"
             | "false"
             | "null"
             | identifier
             | "(" , expression , ")"
             ;


## Parser Notes (for Implementation)

- assignment is right-associative (a = b = c parses as a = (b = c)).
- The left side of assignment must be an lvalue (identifier, deref, or field).
- postfix chains allow calls and field access in any order: f(x).y(z)
- If you later add arrays, add postfix indexing: [ expr ]

