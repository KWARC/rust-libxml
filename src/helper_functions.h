/*
 * This is a small extension to the libxml API.
 * The reason is that we don't convert the structs properly,
 * which prevents some very basic actions
 */
#include <libxml/xmlerror.h>
#include <libxml/tree.h>
#include <libxml/HTMLparser.h>
#include <libxml/xpath.h>
#include <stdlib.h>

/*
 * helper functions for tree
 */

//returns cur->next
xmlNodePtr xmlNextSibling(const xmlNodePtr cur);

//returns cur->prev
xmlNodePtr xmlPrevSibling(const xmlNodePtr cur);

//returns cur->children
xmlNodePtr xmlGetFirstChild(const xmlNodePtr cur);

//returns cur->parent
xmlNodePtr xmlGetParent(const xmlNodePtr cur);

//returns cur->name
const char * xmlNodeGetName(const xmlNodePtr cur);

//returns cur->type
int xmlGetNodeType(const xmlNodePtr cur);

//returns cur->content
//(Different from xmlNodeGetContent)
const char * xmlNodeGetContentPointer(const xmlNodePtr cur);

void setIndentTreeOutput(const int indent);
int getIndentTreeOutput();


/*
 * helper functions for xpath
 */

///returns val->nodesetval->nodeNr
int xmlXPathObjectNumberOfNodes(const xmlXPathObjectPtr val);

///returns val->nodesetval->nodeTab[index]
xmlNodePtr xmlXPathObjectGetNode(const xmlXPathObjectPtr val, size_t index);

///frees the memory of an xmlXPathObject
void xmlFreeXPathObject(xmlXPathObjectPtr val);

/*
 * helper functions for parser
 */
/// Returns well-formed check field of html parser context struct
int htmlWellFormed(const htmlParserCtxtPtr ctxt);

/*
 * helper functions for error handling
 */

void setWellFormednessHandler(const htmlParserCtxtPtr ctxt);
