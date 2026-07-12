from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional


@dataclass
class TSType:
    raw: str


@dataclass
class TSPredefinedType(TSType):
    pass


@dataclass
class TSArrayType(TSType):
    element_type: TSType


@dataclass
class TSGenericType(TSType):
    name: str
    type_args: list[TSType]


@dataclass
class TSUnionType(TSType):
    types: list[TSType]


class TSNode:
    pass


@dataclass
class TSProgram(TSNode):
    statements: list[TSNode]


@dataclass
class TSIdentifier(TSNode):
    name: str
    type_annotation: Optional[TSType] = None


@dataclass
class TSLiteral(TSNode):
    value: str | int | float | bool | None
    raw: str


@dataclass
class TSTemplateLiteral(TSNode):
    parts: list[str | TSNode]


@dataclass
class TSVariableDeclaration(TSNode):
    kind: str
    name: TSIdentifier
    initializer: Optional[TSNode]
    type_annotation: Optional[TSType]


@dataclass
class TSFunctionDeclaration(TSNode):
    name: str
    params: list[TSVariableDeclaration]
    return_type: Optional[TSType]
    body: TSBlockStatement


@dataclass
class TSArrowFunction(TSNode):
    params: list[TSVariableDeclaration]
    return_type: Optional[TSType]
    body: TSBlockStatement | TSNode


@dataclass
class TSBlockStatement(TSNode):
    statements: list[TSNode]


@dataclass
class TSExpressionStatement(TSNode):
    expression: TSNode


@dataclass
class TSReturnStatement(TSNode):
    argument: Optional[TSNode]


@dataclass
class TSIfStatement(TSNode):
    condition: TSNode
    consequence: TSNode
    alternate: Optional[TSNode]


@dataclass
class TSWhileStatement(TSNode):
    condition: TSNode
    body: TSNode


@dataclass
class TSForStatement(TSNode):
    init: Optional[TSNode]
    condition: Optional[TSNode]
    update: Optional[TSNode]
    body: TSNode


@dataclass
class TSBinaryExpression(TSNode):
    operator: str
    left: TSNode
    right: TSNode


@dataclass
class TSUnaryExpression(TSNode):
    operator: str
    argument: TSNode
    prefix: bool


@dataclass
class TSUpdateExpression(TSNode):
    operator: str
    argument: TSNode
    prefix: bool


@dataclass
class TSAssignmentExpression(TSNode):
    operator: str
    left: TSNode
    right: TSNode


@dataclass
class TSCallExpression(TSNode):
    callee: TSNode
    arguments: list[TSNode]
    type_arguments: Optional[list[TSType]] = None


@dataclass
class TSMemberExpression(TSNode):
    object: TSNode
    property: TSNode
    computed: bool


@dataclass
class TSParenthesizedExpression(TSNode):
    expression: TSNode
